//! Declared contracts against counted behaviour.
//!
//! * `BlockPass`: callback calls and the bytes handed over are counted and must equal the
//!   envelope formula exactly; the transform must equal a byte-wise reference.
//! * `FrameDecoder`: `filled` calls per frame are counted under arbitrary read sizes and must
//!   stay within the declared bound, reaching it exactly with one-byte reads.
//! * `GenerationalArena`: shape checks only (its numbers are read from the code).

use core::num::NonZeroU64;
use o1_contract::policy::{NonTemporalWrite, ReuseOnce, Reused, Sequential, Unset};
use o1_contract::{
    Access, Bound, PeakGrowth, Policy, Progress, Quantity::*, ResourceEnvelope as E, Reuse,
    Sharing, Unit, WritePolicy, split_blocks,
};
use o1_core::arena::GenerationalArena;
use o1_core::frame::{FrameDecoder, FrameEncoder, HEADER_LEN, Status};
use o1_core::stream::{BlockPass, LengthMismatch, Pass};
use proptest::prelude::*;

type Streaming<const N: usize> = BlockPass<N, Policy<Sequential, ReuseOnce, NonTemporalWrite>>;
type Plain<const N: usize> = BlockPass<N>;
type ReusedPass<const N: usize> = BlockPass<N, Policy<Unset, Reused, Unset>>;

/// Runs a read and a transform pass, counting what the callbacks receive.
fn observe<const N: usize>(input: &[u8]) -> (Pass, E, Pass, E, Vec<u8>) {
    let (mut calls, mut bytes) = (0u64, 0u64);
    let r = Plain::<N>::new().read(input, |b| {
        calls += 1;
        bytes += b.len() as u64;
    });
    let read_obs = E::ZERO.with(Ops, calls).with(BytesRead, bytes);

    let mut out = vec![0xEEu8; input.len()];
    let (mut calls, mut rd, mut wr) = (0u64, 0u64, 0u64);
    let t = BlockPass::<N>::with(Policy::new().streaming().non_temporal_write())
        .transform(input, &mut out, |i, o| {
            calls += 1;
            rd += i.len() as u64;
            wr += o.len() as u64;
            for (a, b) in i.iter().zip(o.iter_mut()) {
                *b = a.wrapping_mul(3) ^ 0x5a;
            }
        })
        .unwrap();
    let t_obs = E::ZERO
        .with(Ops, calls)
        .with(BytesRead, rd)
        .with(BytesWritten, wr);
    (r, read_obs, t, t_obs, out)
}

fn totals_only(e: E) -> E {
    e.with(WorkingSet, 0)
}

fn check_pass<const N: usize>(input: &[u8]) -> Result<(), TestCaseError> {
    let (r, r_obs, t, t_obs, out) = observe::<N>(input);
    let expect = split_blocks(input.len() as u64, NonZeroU64::new(N as u64).unwrap());
    prop_assert_eq!(
        (r.blocks, r.remainder as u64),
        (expect.full, expect.remainder)
    );
    prop_assert_eq!(t, r);
    // Totals equal the formula exactly.
    let declared_r = Plain::<N>::READ_CONTRACT.envelope(r.blocks).unwrap();
    let declared_t = Streaming::<N>::TRANSFORM_CONTRACT
        .envelope(t.blocks)
        .unwrap();
    prop_assert_eq!(r_obs, totals_only(declared_r));
    prop_assert_eq!(t_obs, totals_only(declared_t));
    prop_assert!(t_obs.within(&declared_t));
    // Working set: streaming = one block in + one out; unstated reuse = the processed extent.
    prop_assert_eq!(
        declared_t.get(WorkingSet),
        if t.blocks == 0 { 0 } else { 2 * N as u64 }
    );
    prop_assert_eq!(declared_r.get(WorkingSet), r.blocks * N as u64);
    // Semantics: every whole-block byte transformed, the remainder untouched.
    let done = input.len() - t.remainder;
    for (k, (&a, &b)) in input.iter().zip(&out).enumerate() {
        let want = if k < done {
            a.wrapping_mul(3) ^ 0x5a
        } else {
            0xEE
        };
        prop_assert_eq!(b, want);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn block_pass_counts_equal_the_formula(input in prop::collection::vec(any::<u8>(), 0..3000)) {
        check_pass::<1>(&input)?;
        check_pass::<3>(&input)?;
        check_pass::<64>(&input)?;
        check_pass::<256>(&input)?;
    }

    #[test]
    fn frame_steps_within_declared_bound(
        len in 0usize..300,
        chunks in prop::collection::vec(1usize..64, 1..32),
    ) {
        let payload: Vec<u8> = (0..len).map(|i| i as u8).collect();
        let mut wire = Vec::new();
        let mut enc = FrameEncoder::new(&payload).unwrap();
        let (h, p) = enc.pending();
        wire.extend_from_slice(h);
        wire.extend_from_slice(p);
        enc.advance(wire.len()).unwrap();

        let mut buf = [0u8; 256];
        let mut dec = FrameDecoder::new(&mut buf).with_discard_limit(512);
        let declared = dec.frame_contract().envelope(len as u64).unwrap();
        let mut rest = &wire[..];
        let mut steps = 0u64;
        let mut k = 0;
        let status = loop {
            let spare = dec.spare().unwrap();
            let n = spare.len().min(rest.len()).min(chunks[k % chunks.len()]);
            k += 1;
            spare[..n].copy_from_slice(&rest[..n]);
            rest = &rest[n..];
            steps += 1;
            match dec.filled(n).unwrap() {
                Status::NeedMore => {}
                s => break s,
            }
        };
        prop_assert!(rest.is_empty());
        prop_assert_eq!(
            status,
            if len <= 256 { Status::Ready(len) } else { Status::Discarded(len) }
        );
        let observed = E::ZERO.with(Ops, steps);
        prop_assert!(observed.within(&declared), "{} steps > {}", steps, declared);
    }
}

#[test]
fn frame_one_byte_reads_reach_the_bound_exactly() {
    for len in [0usize, 1, 5, 255, 256, 300, 512] {
        let payload = vec![9u8; len];
        let enc = FrameEncoder::new(&payload).unwrap();
        let (h, p) = enc.pending();
        let wire: Vec<u8> = h.iter().chain(p).copied().collect();
        let mut buf = [0u8; 256];
        let mut dec = FrameDecoder::new(&mut buf).with_discard_limit(512);
        let contract = dec.frame_contract();
        let mut steps = 0u64;
        for &byte in &wire {
            dec.spare().unwrap()[0] = byte;
            steps += 1;
            if dec.filled(1).unwrap() != Status::NeedMore {
                break;
            }
        }
        assert_eq!(steps, (HEADER_LEN + len) as u64);
        assert_eq!(contract.envelope(len as u64).unwrap().get(Ops), steps);
    }
    // Above max(max_len, discard_limit): no envelope, and the decoder refuses in 4 steps.
    let mut buf = [0u8; 256];
    let dec = FrameDecoder::new(&mut buf).with_discard_limit(512);
    assert!(dec.frame_contract().envelope(513).is_err());
}

#[test]
fn block_pass_examples_and_edges() {
    // 4 MiB in 256-byte blocks: 16 384 executions.
    let input = vec![1u8; 4 << 20];
    assert_eq!(
        Plain::<256>::new().read(&input, |_| {}),
        Pass {
            blocks: 16_384,
            remainder: 0
        }
    );
    // Remainder reported, not processed.
    let p = Plain::<256>::new().read(&input[..1000], |_| {});
    assert_eq!((p.blocks, p.remainder), (3, 232));
    // Zero length.
    assert_eq!(
        Plain::<8>::new().read(&[], |_| unreachable!()),
        Pass {
            blocks: 0,
            remainder: 0
        }
    );
    assert_eq!(Streaming::<8>::TRANSFORM_CONTRACT.envelope(0), Ok(E::ZERO));
    // Length mismatch: nothing processed.
    let mut out = [0u8; 7];
    assert_eq!(
        Plain::<4>::new().transform(&[0u8; 8], &mut out, |_, _| unreachable!()),
        Err(LengthMismatch {
            input: 8,
            output: 7
        })
    );
    // The policy is zero-sized and erased from the value.
    assert_eq!(core::mem::size_of::<Streaming<256>>(), 0);
    assert_eq!(core::mem::size_of::<Plain<256>>(), 0);
}

#[test]
fn block_pass_contract_shape() {
    let t = Streaming::<256>::TRANSFORM_CONTRACT;
    assert_eq!(
        t.bound(),
        Bound::PerUnit(Unit::Block {
            bytes: NonZeroU64::new(256).unwrap()
        })
    );
    assert_eq!(t.progress, Progress::NoSynchronization);
    assert_eq!(t.policy.access, Some(Access::Sequential));
    assert_eq!(t.policy.sharing, Some(Sharing::Exclusive));
    assert_eq!(t.policy.reuse, Some(Reuse::Once));
    assert_eq!(t.policy.write, Some(WritePolicy::NonTemporal));
    assert_eq!(t.cost.peak_growth, PeakGrowth::Fixed);
    assert_eq!(
        Plain::<256>::READ_CONTRACT.policy.sharing,
        Some(Sharing::SharedRead)
    );
    // Reuse changes the reported working set, not the code.
    let reused = ReusedPass::<256>::TRANSFORM_CONTRACT
        .envelope(1000)
        .unwrap();
    let streamed = Streaming::<256>::TRANSFORM_CONTRACT.envelope(1000).unwrap();
    assert_eq!(reused.get(WorkingSet), 512_000);
    assert_eq!(streamed.get(WorkingSet), 512);
    assert_eq!(totals_only(reused), totals_only(streamed));
}

#[test]
fn arena_contract_shape() {
    type A = GenerationalArena<u64, 16>;
    for (c, sharing, rd, wr) in [
        (A::INSERT_CONTRACT, Sharing::Exclusive, 0, 8),
        (A::REMOVE_CONTRACT, Sharing::Exclusive, 8, 0),
        (A::GET_CONTRACT, Sharing::SharedRead, 0, 0),
    ] {
        assert_eq!(c.bound(), Bound::Constant);
        assert_eq!(c.progress, Progress::NoSynchronization);
        assert_eq!(c.policy.access, Some(Access::Random));
        assert_eq!(c.policy.sharing, Some(sharing));
        let e = c.envelope(0).unwrap();
        assert_eq!(e.get(Ops), 1);
        assert_eq!((e.get(BytesRead), e.get(BytesWritten)), (rd, wr));
        assert_eq!(e.get(WorkingSet), core::mem::size_of::<A>() as u64);
        for q in [
            AtomicLoads,
            AtomicStores,
            AtomicRmws,
            SharedReads,
            SharedWrites,
            KernelEntries,
            Allocations,
        ] {
            assert_eq!(e.get(q), 0);
        }
    }
}

/// The archive-style example of `docs/EXECUTION_CONTRACTS.md` §13, computed: a 4 GiB
/// sequential extent, 1 MiB reads and writes, a 256-byte streaming transform.
#[test]
fn archive_style_example_numbers() {
    const GIB: u64 = 1 << 30;
    const MIB: u64 = 1 << 20;
    let blocks = split_blocks(4 * GIB, NonZeroU64::new(256).unwrap());
    assert_eq!((blocks.full, blocks.remainder), (16_777_216, 0));
    let io_per_chunk = E::ZERO.with(KernelEntries, 2); // one read, one write: caller's envelope
    let hot_state = E::ZERO.with(WorkingSet, 4096);
    let job = Streaming::<256>::TRANSFORM_CONTRACT
        .envelope(blocks.full)
        .unwrap()
        .alongside(io_per_chunk.repeat(4 * GIB / MIB).unwrap())
        .unwrap()
        .alongside(hot_state)
        .unwrap();
    assert_eq!(
        job.to_string(),
        "ops=16777216, bytes_read=4294967296, bytes_written=4294967296, \
         kernel_entries=8192, working_set=4608"
    );
}
