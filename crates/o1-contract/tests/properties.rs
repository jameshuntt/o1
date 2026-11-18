//! Property tests: envelope arithmetic against a `u128` reference, algebraic laws,
//! size multipliers against brute force.

use core::num::NonZeroU64;
use o1_contract::{
    Cost, EnvelopeError, Kind, PeakGrowth, Quantity, ResourceEnvelope as E, Unit, lines_touched,
    split_blocks,
};
use proptest::prelude::*;

const N: usize = Quantity::COUNT;

fn env(v: [u64; N]) -> E {
    Quantity::ALL
        .iter()
        .zip(v)
        .fold(E::ZERO, |e, (&q, x)| e.with(q, x))
}

fn arr(e: &E) -> [u64; N] {
    Quantity::ALL.map(|q| e.get(q))
}

/// Any counts, including values near `u64::MAX` (to reach overflow).
fn any_env() -> impl Strategy<Value = E> {
    prop::array::uniform11(prop_oneof![
        3 => 0u64..1000,
        1 => any::<u64>(),
        1 => (u64::MAX - 1000)..=u64::MAX,
    ])
    .prop_map(env)
}

/// Counts small enough that sums of three and products by `u32` never overflow.
fn small_env() -> impl Strategy<Value = E> {
    prop::array::uniform11(0u64..(1 << 28)).prop_map(env)
}

/// Reference combination in `u128`: `None` iff some result exceeds `u64::MAX`; the error
/// names the first such quantity in `Quantity::ALL` order.
fn ref_combine(a: &E, b: &E, peaks_add: bool) -> Result<E, EnvelopeError> {
    let mut out = [0u64; N];
    for (i, q) in Quantity::ALL.iter().enumerate() {
        let (x, y) = (a.get(*q) as u128, b.get(*q) as u128);
        let r = if q.kind() == Kind::Peak && !peaks_add {
            x.max(y)
        } else {
            x + y
        };
        out[i] = u64::try_from(r).map_err(|_| EnvelopeError::Overflow { quantity: *q })?;
    }
    Ok(env(out))
}

fn ref_scale(a: &E, n: u64, peaks_scale: bool) -> Result<E, EnvelopeError> {
    if n == 0 {
        return Ok(E::ZERO);
    }
    let mut out = [0u64; N];
    for (i, q) in Quantity::ALL.iter().enumerate() {
        let x = a.get(*q) as u128;
        let r = if q.kind() == Kind::Peak && !peaks_scale {
            x
        } else {
            x * n as u128
        };
        out[i] = u64::try_from(r).map_err(|_| EnvelopeError::Overflow { quantity: *q })?;
    }
    Ok(env(out))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn combine_matches_u128_reference(a in any_env(), b in any_env()) {
        prop_assert_eq!(a.then(b), ref_combine(&a, &b, false));
        prop_assert_eq!(a.alongside(b), ref_combine(&a, &b, true));
    }

    #[test]
    fn scale_matches_u128_reference(a in any_env(), n in prop_oneof![0u64..4, any::<u64>()]) {
        prop_assert_eq!(a.repeat(n), ref_scale(&a, n, false));
        prop_assert_eq!(a.repeat_retained(n), ref_scale(&a, n, true));
    }

    #[test]
    fn overflow_never_yields_a_smaller_value(a in any_env(), b in any_env(), n in 1u64..u64::MAX) {
        for r in [a.then(b), a.alongside(b)].into_iter().flatten() {
            prop_assert!(a.within(&r) && b.within(&r));
        }
        for r in [a.repeat(n), a.repeat_retained(n)].into_iter().flatten() {
            prop_assert!(a.within(&r));
        }
    }

    #[test]
    fn commutative_including_errors(a in any_env(), b in any_env()) {
        prop_assert_eq!(a.then(b), b.then(a));
        prop_assert_eq!(a.alongside(b), b.alongside(a));
    }

    #[test]
    fn associative(a in small_env(), b in small_env(), c in small_env()) {
        prop_assert_eq!(a.then(b).unwrap().then(c), a.then(b.then(c).unwrap()));
        prop_assert_eq!(
            a.alongside(b).unwrap().alongside(c),
            a.alongside(b.alongside(c).unwrap())
        );
    }

    #[test]
    fn scaling_distributes(a in small_env(), b in small_env(), m in 0u64..1000, n in 0u64..1000) {
        // n × (a then b) = (n × a) then (n × b); likewise for the retained forms.
        prop_assert_eq!(
            a.then(b).unwrap().repeat(n),
            a.repeat(n).unwrap().then(b.repeat(n).unwrap())
        );
        prop_assert_eq!(
            a.alongside(b).unwrap().repeat_retained(n),
            a.repeat_retained(n).unwrap().alongside(b.repeat_retained(n).unwrap())
        );
        // (m + n) × a = (m × a) then (n × a); retained: alongside.
        prop_assert_eq!(a.repeat(m + n), a.repeat(m).unwrap().then(a.repeat(n).unwrap()));
        prop_assert_eq!(
            a.repeat_retained(m + n),
            a.repeat_retained(m).unwrap().alongside(a.repeat_retained(n).unwrap())
        );
    }

    #[test]
    fn peaks_max_or_sum_totals_sum(a in small_env(), b in small_env(), n in 1u64..1000) {
        let ws = Quantity::WorkingSet;
        prop_assert_eq!(a.then(b).unwrap().get(ws), a.get(ws).max(b.get(ws)));
        prop_assert_eq!(a.alongside(b).unwrap().get(ws), a.get(ws) + b.get(ws));
        prop_assert_eq!(a.repeat(n).unwrap().get(ws), a.get(ws));
        prop_assert_eq!(a.repeat_retained(n).unwrap().get(ws), a.get(ws) * n);
        for q in Quantity::ALL.into_iter().filter(|q| q.kind() == Kind::Total) {
            prop_assert_eq!(a.then(b).unwrap().get(q), a.get(q) + b.get(q));
            prop_assert_eq!(a.repeat(n).unwrap().get(q), a.get(q) * n);
        }
    }

    #[test]
    fn monotone(a in small_env(), b in small_env(), extra in small_env(), n in 1u64..1000) {
        let a2 = a.alongside(extra).unwrap(); // a ≤ a2
        prop_assert!(a.within(&a2));
        prop_assert!(a.then(b).unwrap().within(&a2.then(b).unwrap()));
        prop_assert!(a.alongside(b).unwrap().within(&a2.alongside(b).unwrap()));
        prop_assert!(a.repeat(n).unwrap().within(&a.repeat(n + 1).unwrap()));
        prop_assert!(a.repeat(n).unwrap().within(&a2.repeat(n).unwrap()));
        prop_assert!(a.repeat(n).unwrap().within(&a.repeat_retained(n).unwrap()));
    }

    #[test]
    fn within_is_a_partial_order(a in any_env(), b in any_env(), x in small_env()) {
        prop_assert!(a.within(&a));
        if a.within(&b) && b.within(&a) {
            prop_assert_eq!(a, b);
        }
        let s = small_env_add(&a, &x);
        if let Some(s) = s {
            if let Ok(t) = s.alongside(x) {
                prop_assert!(a.within(&s) && s.within(&t) && a.within(&t));
            }
        }
        // check_within names the first quantity above the limit.
        match a.check_within(&b) {
            Ok(()) => prop_assert!(Quantity::ALL.iter().all(|&q| a.get(q) <= b.get(q))),
            Err(e) => {
                let first = Quantity::ALL.into_iter().find(|&q| a.get(q) > b.get(q)).unwrap();
                prop_assert_eq!(e.quantity, first);
                prop_assert_eq!((e.value, e.limit), (a.get(first), b.get(first)));
            }
        }
    }

    #[test]
    fn split_blocks_never_drops_a_byte(len in any::<u64>(), block in 1u64..=u64::MAX) {
        let b = split_blocks(len, NonZeroU64::new(block).unwrap());
        prop_assert!(b.remainder < block);
        prop_assert_eq!(b.full as u128 * block as u128 + b.remainder as u128, len as u128);
        let cover = b.covering() as u128;
        prop_assert!(cover * block as u128 >= len as u128);
        prop_assert!(cover == b.full as u128 || cover == b.full as u128 + 1);
    }

    #[test]
    fn lines_touched_matches_brute_force(offset in 0u64..1000, len in 0u64..2000, line in 1u64..300) {
        let lines: std::collections::BTreeSet<u64> =
            (offset..offset + len).map(|byte| byte / line).collect();
        prop_assert_eq!(
            lines_touched(offset, len, NonZeroU64::new(line).unwrap()),
            lines.len() as u64
        );
    }

    #[test]
    fn lines_touched_bounds(offset in any::<u64>(), len in any::<u64>(), line in 1u64..=u64::MAX) {
        let nz = NonZeroU64::new(line).unwrap();
        let t = lines_touched(offset, len, nz);
        let aligned = len.div_ceil(line);
        prop_assert!(t == aligned || t == aligned + 1);
        prop_assert_eq!(lines_touched(offset - offset % line, len, nz), aligned);
        if len > 0 {
            prop_assert!(t >= 1);
        }
    }

    #[test]
    fn cost_at_composes_fixed_alongside_per_unit(
        fixed in small_env(), per in small_env(), units in 0u64..100_000,
        max in prop::option::of(0u64..100_000), retained in any::<bool>(),
    ) {
        let cost = Cost {
            fixed,
            per_unit: per,
            unit: Unit::Item,
            max_units: max,
            peak_growth: if retained { PeakGrowth::PerUnit } else { PeakGrowth::Fixed },
        };
        let got = cost.at(units);
        match max {
            Some(m) if units > m => {
                prop_assert_eq!(got, Err(EnvelopeError::AboveBound { units, max: m }));
            }
            _ => {
                let scaled = ref_scale(&per, units, retained).unwrap();
                prop_assert_eq!(got, ref_combine(&fixed, &scaled, true));
                prop_assert_eq!(got, cost.at(units)); // deterministic
            }
        }
    }
}

fn small_env_add(a: &E, x: &E) -> Option<E> {
    a.alongside(*x).ok()
}

#[test]
fn zero_work() {
    let one = E::ZERO
        .with(Quantity::Ops, 1)
        .with(Quantity::WorkingSet, 64);
    assert_eq!(one.repeat(0), Ok(E::ZERO));
    assert_eq!(one.repeat_retained(0), Ok(E::ZERO));
    assert_eq!(E::ZERO.then(E::ZERO), Ok(E::ZERO));
    let cost = Cost {
        fixed: E::ZERO.with(Quantity::WorkingSet, 8),
        per_unit: one,
        unit: Unit::Block {
            bytes: NonZeroU64::new(256).unwrap(),
        },
        max_units: None,
        peak_growth: PeakGrowth::Fixed,
    };
    assert_eq!(
        cost.for_extent(17, 0),
        Ok(E::ZERO.with(Quantity::WorkingSet, 8))
    );
    assert_eq!(lines_touched(u64::MAX, 0, NonZeroU64::MIN), 0);
    assert_eq!(split_blocks(0, NonZeroU64::MIN).covering(), 0);
}

#[test]
fn maximum_representable_work() {
    for q in Quantity::ALL {
        let max = E::ZERO.with(q, u64::MAX);
        assert_eq!(max.repeat(1), Ok(max));
        assert_eq!(max.then(E::ZERO), Ok(max));
        assert_eq!(max.alongside(E::ZERO), Ok(max));
        let one = E::ZERO.with(q, 1);
        assert_eq!(
            max.alongside(one),
            Err(EnvelopeError::Overflow { quantity: q })
        );
        assert_eq!(
            max.repeat_retained(2),
            Err(EnvelopeError::Overflow { quantity: q })
        );
        if q.kind() == Kind::Peak {
            assert_eq!(max.then(one), Ok(max)); // max never overflows
            assert_eq!(max.repeat(u64::MAX), Ok(max));
        } else {
            assert_eq!(max.then(one), Err(EnvelopeError::Overflow { quantity: q }));
            assert_eq!(max.repeat(2), Err(EnvelopeError::Overflow { quantity: q }));
        }
    }
    // The largest extents convert without overflow.
    let nz = |n| NonZeroU64::new(n).unwrap();
    assert_eq!(lines_touched(u64::MAX, u64::MAX, nz(1)), u64::MAX);
    assert_eq!(lines_touched(63, u64::MAX, nz(64)), u64::MAX / 64 + 2);
    assert_eq!(split_blocks(u64::MAX, nz(2)).covering(), 1 << 63);
}

#[test]
fn item_units_are_not_an_extent() {
    let cost = Cost::constant(E::ZERO.with(Quantity::Ops, 1));
    assert_eq!(cost.for_extent(0, 10), Err(EnvelopeError::NotAnExtentUnit));
    assert_eq!(cost.at(0), Ok(E::ZERO.with(Quantity::Ops, 1)));
}

#[test]
fn display_lists_non_zero_quantities() {
    assert_eq!(E::ZERO.to_string(), "zero");
    let e = E::ZERO.with(Quantity::Ops, 2).with(Quantity::WorkingSet, 9);
    assert_eq!(e.to_string(), "ops=2, working_set=9");
    assert_eq!(arr(&e)[0], 2);
}
