//! Prediction intervals (design §11): the interval of one calibration point over the runs
//! supplied, and the compositions (`Seq` adds, `Repeat` multiplies, `Par` takes the critical
//! path, `Branch` weighs). Nothing is added: no fixed percentage, no safety factor.

use alloc::string::String;
use alloc::vec::Vec;

use crate::refusal::Refusal;

/// `[lower, upper]` with a central value, in ns (per unit, or for a whole leaf or plan).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Interval {
    /// Lower end.
    pub lower: f64,
    /// Central value.
    pub central: f64,
    /// Upper end.
    pub upper: f64,
}

/// One run's numbers at one calibration point (ns per unit).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RunPoint {
    /// Run index in the evidence.
    pub run: usize,
    /// Calibration id.
    pub calibration: String,
    /// Pooled median `M_r`.
    pub median: f64,
    /// Spread `s_r`.
    pub spread: f64,
    /// Smallest per-round median `lo_r`.
    pub round_low: f64,
    /// Largest per-round median `hi_r`.
    pub round_high: f64,
    /// Stable in its run.
    pub stable: bool,
}

/// Median of a non-empty slice (mean of the two middle values for an even length).
pub(crate) fn median(v: &[f64]) -> f64 {
    let mut s: Vec<f64> = v.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    if n == 0 {
        return f64::NAN;
    }
    if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    }
}

impl Interval {
    /// The interval of one calibration point over `runs` (§11):
    /// `lower = min_r min(lo_r, M_r(1 − s_r))`, `upper = max_r max(hi_r, M_r(1 + s_r))`,
    /// `central = median of M_r`. `None` for no run.
    pub fn of_point(runs: &[RunPoint]) -> Option<Self> {
        if runs.is_empty() {
            return None;
        }
        let mut lower = f64::INFINITY;
        let mut upper = f64::NEG_INFINITY;
        let mut meds = Vec::with_capacity(runs.len());
        for r in runs {
            lower = lower.min(r.round_low.min(r.median * (1.0 - r.spread)));
            upper = upper.max(r.round_high.max(r.median * (1.0 + r.spread)));
            meds.push(r.median);
        }
        Some(Self {
            lower,
            central: median(&meds),
            upper,
        })
    }

    /// `[min lower, max upper]` of two intervals, with `central` given.
    pub fn hull(a: &Self, b: &Self, central: f64) -> Self {
        Self {
            lower: a.lower.min(b.lower),
            central,
            upper: a.upper.max(b.upper),
        }
    }

    /// `count ×` each end (the only `u64 → f64` conversion of a count).
    ///
    /// # Errors
    ///
    /// [`Refusal::Overflow`] when a product is not finite.
    pub fn times(&self, count: u64) -> Result<Self, Refusal> {
        let c = count as f64;
        finite(Self {
            lower: self.lower * c,
            central: self.central * c,
            upper: self.upper * c,
        })
    }

    /// `Seq`: each end adds.
    ///
    /// # Errors
    ///
    /// [`Refusal::Overflow`] when a sum is not finite.
    pub fn then(&self, other: &Self) -> Result<Self, Refusal> {
        finite(Self {
            lower: self.lower + other.lower,
            central: self.central + other.central,
            upper: self.upper + other.upper,
        })
    }

    /// `Par` of independent branches: the critical path, `[max lower, max upper]`, central =
    /// max central. Never the sum. `None` for no branch.
    pub fn critical_path(items: &[Self]) -> Option<Self> {
        let first = items.first()?;
        let mut out = *first;
        for i in &items[1..] {
            out.lower = out.lower.max(i.lower);
            out.central = out.central.max(i.central);
            out.upper = out.upper.max(i.upper);
        }
        Some(out)
    }

    /// `(upper − lower) / central`.
    pub fn relative_width(&self) -> f64 {
        (self.upper - self.lower) / self.central
    }

    /// Whether `x` lies in `[lower, upper]`.
    pub fn contains(&self, x: f64) -> bool {
        self.lower <= x && x <= self.upper
    }
}

fn finite(i: Interval) -> Result<Interval, Refusal> {
    if i.lower.is_finite() && i.central.is_finite() && i.upper.is_finite() {
        Ok(i)
    } else {
        Err(Refusal::Overflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn rp(m: f64, s: f64, lo: f64, hi: f64) -> RunPoint {
        RunPoint {
            run: 0,
            calibration: "c".into(),
            median: m,
            spread: s,
            round_low: lo,
            round_high: hi,
            stable: true,
        }
    }

    #[test]
    fn one_run() {
        // spread wider than the round range: the spread decides both ends.
        let i = Interval::of_point(&[rp(10.0, 0.1, 9.5, 10.5)]).unwrap();
        assert_eq!(i.central, 10.0);
        assert!((i.lower - 9.0).abs() < 1e-12);
        assert!((i.upper - 11.0).abs() < 1e-12);
        // round range wider than the spread: the round medians decide.
        let i = Interval::of_point(&[rp(10.0, 0.01, 8.0, 13.0)]).unwrap();
        assert_eq!((i.lower, i.upper), (8.0, 13.0));
    }

    #[test]
    fn two_runs_widen_by_their_disagreement() {
        let a = rp(10.0, 0.01, 9.95, 10.05);
        let b = rp(12.0, 0.01, 11.9, 12.1);
        let i = Interval::of_point(&[a, b]).unwrap();
        assert_eq!(i.central, 11.0);
        assert!((i.lower - 9.9).abs() < 1e-12);
        assert!((i.upper - 12.12).abs() < 1e-12);
        assert!(Interval::of_point(&[]).is_none());
    }

    #[test]
    fn composition() {
        let a = Interval {
            lower: 1.0,
            central: 2.0,
            upper: 3.0,
        };
        let b = Interval {
            lower: 10.0,
            central: 20.0,
            upper: 30.0,
        };
        let s = a.then(&b).unwrap();
        assert_eq!((s.lower, s.central, s.upper), (11.0, 22.0, 33.0));
        let p = Interval::critical_path(&[a, b]).unwrap();
        assert_eq!(p, b);
        let mixed = Interval {
            lower: 15.0,
            central: 16.0,
            upper: 17.0,
        };
        let p = Interval::critical_path(&[b, mixed]).unwrap();
        assert_eq!((p.lower, p.central, p.upper), (15.0, 20.0, 30.0));
        assert!(Interval::critical_path(&[]).is_none());
        let r = a.times(4).unwrap();
        assert_eq!((r.lower, r.central, r.upper), (4.0, 8.0, 12.0));
        assert_eq!(a.relative_width(), 1.0);
        assert!(a.contains(1.0) && a.contains(3.0) && !a.contains(3.0001));
    }

    #[test]
    fn non_finite_is_overflow() {
        let a = Interval {
            lower: 1.0,
            central: 1e300,
            upper: 1e308,
        };
        assert_eq!(a.times(u64::MAX), Err(Refusal::Overflow));
        assert_eq!(a.then(&a), Err(Refusal::Overflow));
    }

    proptest! {
        #[test]
        fn point_interval_contains_every_round_median_and_spread(
            runs in proptest::collection::vec((0.01f64..1e6, 0.0f64..0.9, 0.5f64..1.0, 1.0f64..1.5), 1..5)
        ) {
            let pts: Vec<RunPoint> = runs.iter().map(|(m, s, l, h)| rp(*m, *s, m * l, m * h)).collect();
            let i = Interval::of_point(&pts).unwrap();
            for p in &pts {
                prop_assert!(i.lower <= p.round_low && p.round_high <= i.upper);
                prop_assert!(i.lower <= p.median * (1.0 - p.spread));
                prop_assert!(p.median * (1.0 + p.spread) <= i.upper);
                prop_assert!(i.lower <= i.central && i.central <= i.upper);
            }
        }

        #[test]
        fn seq_is_additive_and_par_is_never_more_than_seq(
            a in (0.0f64..1e6, 0.0f64..1e6, 0.0f64..1e6),
            b in (0.0f64..1e6, 0.0f64..1e6, 0.0f64..1e6),
        ) {
            let mk = |t: (f64, f64, f64)| {
                let mut v = [t.0, t.1, t.2];
                v.sort_by(f64::total_cmp);
                Interval { lower: v[0], central: v[1], upper: v[2] }
            };
            let (x, y) = (mk(a), mk(b));
            let s = x.then(&y).unwrap();
            let p = Interval::critical_path(&[x, y]).unwrap();
            prop_assert!(p.upper <= s.upper && p.lower <= s.lower + 1e-9 && p.central <= s.central);
        }
    }
}
