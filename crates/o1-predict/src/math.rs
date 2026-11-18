//! The two real functions the predictor needs, without `std` (core has no `log2` or `sqrt` for
//! `f64` at MSRV 1.85): `log2` of a positive integer and the square root of a positive number.
//! Both are only used on measured values and geometry, never on counts.

/// |x|.
pub(crate) fn abs(x: f64) -> f64 {
    if x < 0.0 { -x } else { x }
}

/// `2^k` for `k` in `0..=1023`, exactly.
fn pow2(k: u32) -> f64 {
    f64::from_bits(u64::from(1023 + k) << 52)
}

/// `log2(x)` for `x ≥ 1`: `x = 2^k · m`, `m ∈ [1, 2)`, `ln m = 2·atanh((m − 1)/(m + 1))` by its
/// series (|z| ≤ 1/3, so 40 terms are far below `f64` resolution).
pub(crate) fn log2_u64(x: u64) -> f64 {
    if x == 0 {
        return f64::NEG_INFINITY;
    }
    let k = 63 - x.leading_zeros();
    let m = x as f64 / pow2(k);
    let z = (m - 1.0) / (m + 1.0);
    let z2 = z * z;
    let mut term = z;
    let mut sum = 0.0;
    let mut n = 1.0;
    for _ in 0..40 {
        sum += term / n;
        term *= z2;
        n += 2.0;
    }
    f64::from(k) + 2.0 * sum / core::f64::consts::LN_2
}

/// √x for finite `x ≥ 0` (Newton from an exponent-halved first guess; NaN for negative input).
pub(crate) fn sqrt(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 || x.is_infinite() {
        return x;
    }
    let mut y = f64::from_bits((x.to_bits() >> 1) + (1023u64 << 51));
    for _ in 0..64 {
        let next = 0.5 * (y + x / y);
        if next == y {
            break;
        }
        y = next;
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn exact_powers() {
        assert_eq!(log2_u64(1), 0.0);
        assert_eq!(log2_u64(1 << 20), 20.0);
        assert_eq!(log2_u64(1 << 63), 63.0);
        assert_eq!(sqrt(16.0), 4.0);
        assert_eq!(sqrt(0.0), 0.0);
        assert!(sqrt(-1.0).is_nan());
        assert_eq!(abs(-2.5), 2.5);
    }

    proptest! {
        #[test]
        fn log2_agrees_with_std(x in 1u64..u64::MAX) {
            let a = log2_u64(x);
            let b = (x as f64).log2();
            prop_assert!((a - b).abs() <= 1e-12 * b.abs().max(1.0), "{} vs {}", a, b);
        }

        #[test]
        fn sqrt_agrees_with_std(x in 1e-300f64..1e300) {
            let a = sqrt(x);
            let b = x.sqrt();
            prop_assert!((a - b).abs() <= 1e-15 * b, "{} vs {}", a, b);
        }
    }
}
