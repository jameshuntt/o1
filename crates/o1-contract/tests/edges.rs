//! Edge cases at the limits of `u64`, written during the independent audit of unit 2.
//! Each one is a way a giant workload could have been turned into a small estimate.

use core::num::NonZeroU64;

use o1_contract::{
    Cost, EnvelopeError, PeakGrowth, Quantity, Quantity::*, ResourceEnvelope as E, Unit,
    lines_touched, split_blocks,
};

fn nz(n: u64) -> NonZeroU64 {
    NonZeroU64::new(n).unwrap()
}

fn per_block(bytes: u64, growth: PeakGrowth) -> Cost {
    Cost {
        fixed: E::ZERO.with(Ops, 1).with(WorkingSet, 4096),
        per_unit: E::ZERO
            .with(Ops, 1)
            .with(BytesRead, bytes)
            .with(WorkingSet, bytes),
        unit: Unit::Block { bytes: nz(bytes) },
        max_units: None,
        peak_growth: growth,
    }
}

#[test]
fn the_largest_extent_is_an_error_or_exact_never_small() {
    // u64::MAX bytes in 256-byte blocks: the block count fits, bytes read would not
    // (covering blocks × 256 > u64::MAX), so the answer is an overflow naming BytesRead.
    let cost = per_block(256, PeakGrowth::Fixed);
    assert_eq!(
        cost.for_extent(0, u64::MAX),
        Err(EnvelopeError::Overflow {
            quantity: BytesRead
        })
    );
    // The largest extent that is a whole number of blocks is exact.
    let whole = u64::MAX - (u64::MAX % 256);
    let e = cost.for_extent(0, whole).unwrap();
    assert_eq!(e.get(BytesRead), whole);
    assert_eq!(e.get(Ops), 1 + whole / 256);
    assert_eq!(e.get(WorkingSet), 4096 + 256);
}

#[test]
fn fixed_plus_scaled_overflow_is_reported() {
    // per-unit × units fits, adding the fixed part does not.
    let cost = Cost {
        fixed: E::ZERO.with(Ops, 2),
        per_unit: E::ZERO.with(Ops, 1),
        unit: Unit::Byte,
        max_units: None,
        peak_growth: PeakGrowth::Fixed,
    };
    assert!(cost.at(u64::MAX - 2).is_ok());
    assert_eq!(
        cost.at(u64::MAX - 1),
        Err(EnvelopeError::Overflow { quantity: Ops })
    );
}

#[test]
fn retained_peak_overflow_is_reported() {
    let cost = per_block(1 << 40, PeakGrowth::PerUnit);
    // 2^24 blocks of 2^40 bytes: the working set alone is 2^64.
    assert!(matches!(
        cost.at(1 << 24),
        Err(EnvelopeError::Overflow { .. })
    ));
    // The same job with a fixed peak has a working set of one block.
    let fixed = Cost {
        per_unit: E::ZERO.with(Ops, 1).with(WorkingSet, 1 << 40),
        ..per_block(1 << 40, PeakGrowth::Fixed)
    };
    assert_eq!(
        fixed.at(1 << 24).unwrap().get(WorkingSet),
        4096 + (1u64 << 40)
    );
}

#[test]
fn zero_units_keeps_the_fixed_part_only() {
    let cost = per_block(256, PeakGrowth::PerUnit);
    let e = cost.at(0).unwrap();
    assert_eq!(e, cost.fixed);
    assert_eq!(cost.for_extent(7, 0).unwrap(), cost.fixed);
}

#[test]
fn remainder_is_one_more_block_at_every_size() {
    for block in [1u64, 2, 3, 255, 256, 257, 1 << 20, u64::MAX] {
        for len in [0u64, 1, block - 1, block, block.saturating_add(1), u64::MAX] {
            let b = split_blocks(len, nz(block));
            // Nothing is lost: the pieces rebuild the length exactly, in u128.
            assert_eq!(
                b.full as u128 * block as u128 + b.remainder as u128,
                len as u128
            );
            assert!(b.remainder < block);
            let covering = b.covering() as u128;
            assert!(covering * block as u128 >= len as u128);
            assert!(covering.saturating_sub(1) * (block as u128) < len.max(1) as u128);
        }
    }
}

#[test]
fn lines_at_the_limits() {
    let m = u64::MAX;
    assert_eq!(lines_touched(m, m, nz(1)), m);
    assert_eq!(lines_touched(0, m, nz(m)), 1);
    assert_eq!(lines_touched(1, m, nz(m)), 2);
    assert_eq!(lines_touched(m, 1, nz(64)), 1); // u64::MAX % 64 = 63: the last byte of a line
    assert_eq!(lines_touched(m, 2, nz(64)), 2);
    // A reference computed without the function, over a small exhaustive range.
    for line in 1u64..=9 {
        for offset in 0u64..20 {
            for len in 0u64..40 {
                let expect = if len == 0 {
                    0
                } else {
                    (offset % line + len - 1) / line + 1
                };
                assert_eq!(lines_touched(offset, len, nz(line)), expect);
            }
        }
    }
}

#[test]
fn sequential_and_side_by_side_differ_only_in_the_peak() {
    let a = E::ZERO.with(Ops, 3).with(WorkingSet, 100);
    let b = E::ZERO.with(Ops, 4).with(WorkingSet, 60);
    let seq = a.then(b).unwrap();
    let par = a.alongside(b).unwrap();
    for q in Quantity::ALL {
        if q == WorkingSet {
            assert_eq!((seq.get(q), par.get(q)), (100, 160));
        } else {
            assert_eq!(seq.get(q), par.get(q));
        }
    }
    // A peak never overflows under `then`, and does under `alongside`.
    let big = E::ZERO.with(WorkingSet, u64::MAX);
    assert_eq!(big.then(big).unwrap().get(WorkingSet), u64::MAX);
    assert!(big.alongside(big).is_err());
}
