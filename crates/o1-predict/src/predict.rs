//! `predict` walks a plan and composes; the selector chooses evidence (design §13). Revision A
//! refuses regime boundaries; revision B (`allow_bracket`) brackets them, informational only.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::class::OperationClass;
use crate::evidence::{Evidence, Now};
use crate::geometry::Geometry;
use crate::identity::Implementation;
use crate::interval::Interval;
use crate::plan::{Basis, Count, Leaf, Plan, Unknown};
use crate::refusal::{ConcurrencyReason, PlanRefusal, Refusal};
use crate::selector::{self, Curve, Placement};

/// The predictor's method id (recorded in every prediction).
pub const METHOD: &str = "o1-predict.operation-class.v1";

/// The width at or below which an otherwise admissible prediction is graded `Admission`
/// (a label boundary, not a pass threshold; design §11).
pub const ADMISSION_WIDTH: f64 = 0.20;

/// Prediction options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Options {
    /// Bracket a regime boundary instead of refusing it (revision B).
    pub allow_bracket: bool,
}

impl Options {
    /// Revision A: boundaries refused.
    pub const CLASS_A: Options = Options {
        allow_bracket: false,
    };
    /// Revision B: bracketing allowed.
    pub const CLASS_B: Options = Options {
        allow_bracket: true,
    };

    /// `A` or `B`.
    pub fn revision(&self) -> &'static str {
        if self.allow_bracket { "B" } else { "A" }
    }
}

/// How much a prediction may be relied on.
///
/// `Admission` names structural criteria of the evidence; it does not by itself establish
/// that a prediction is suitable for admission or placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Grade {
    /// Every leaf measured or interpolated in one regime, every supporting calibration stable
    /// in every run, at least two runs supplied and agreeing, exact counts, no `Par`, no
    /// `Branch`, and `(upper − lower) / central ≤ 0.20`.
    Admission,
    /// Anything else that is not refused.
    Informational,
}

/// A leaf's placement, as recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PlacementKind {
    /// At a measured point.
    Measured,
    /// Between two points of one regime.
    Interpolated,
    /// Between two points across a regime boundary (revision B only).
    Bracketed,
}

/// The evidence of one leaf.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LeafPrediction {
    /// Leaf name.
    pub name: String,
    /// Class.
    pub class: OperationClass,
    /// Geometry.
    pub geometry: Geometry,
    /// Placement.
    pub placement: PlacementKind,
    /// The supporting points are one regime (always true for `Measured` and `Interpolated`).
    pub one_regime: bool,
    /// The supporting points' values.
    pub points: Vec<u64>,
    /// Calibration ids (every run, every supporting point).
    pub calibrations: Vec<String>,
    /// The code measured.
    pub implementation: Implementation,
    /// Unit of the count.
    pub unit: String,
    /// The weakest count basis on the path to this leaf.
    pub basis: Basis,
    /// Units executed (leaf count × enclosing repetitions).
    pub count: u64,
    /// ns per unit.
    pub per_unit: Interval,
    /// ns for the leaf.
    pub total: Interval,
    /// Every supporting point agrees between every pair of runs (repeatability rule).
    pub runs_agree: bool,
}

/// One profile run a prediction rests on.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RunRef {
    /// Identity digest.
    pub identity_digest: String,
    /// Environment digest.
    pub environment_digest: String,
    /// Revision.
    pub revision: String,
    /// Method digest.
    pub method_digest: String,
    /// Conditions summary.
    pub conditions: String,
}

/// A prediction: an interval with everything it rests on.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Prediction {
    /// [`METHOD`].
    pub method: String,
    /// Predictor revision (`A`: boundaries refused, `B`: bracketing allowed).
    pub revision: String,
    /// Digest of the plan.
    pub plan_digest: String,
    /// ns for the whole plan.
    pub interval: Interval,
    /// `(upper − lower) / central`.
    pub relative_width: f64,
    /// Grade.
    pub grade: Grade,
    /// Weakest count basis.
    pub basis: Basis,
    /// Per leaf.
    pub leaves: Vec<LeafPrediction>,
    /// The runs.
    pub runs: Vec<RunRef>,
    /// What the prediction does not model or reports.
    pub notes: Vec<String>,
}

/// One path of a plan with branches.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PathPrediction {
    /// The arm chosen at each branch, in walk order.
    pub choices: Vec<usize>,
    /// The prediction of that path, or its refusal.
    pub result: Result<Prediction, PlanRefusal>,
}

struct Walk<'a> {
    evidence: &'a Evidence,
    now: &'a Now,
    options: Options,
    leaves: Vec<LeafPrediction>,
    notes: Vec<String>,
    has_par: bool,
    has_branch: bool,
}

fn agree_all(curve: &Curve, placement: &Placement) -> bool {
    let idx: Vec<usize> = match *placement {
        Placement::Measured { index } => alloc::vec![index],
        Placement::Interpolated { below } | Placement::Bracketed { below } => {
            alloc::vec![below, below + 1]
        }
    };
    idx.iter().filter_map(|i| curve.points.get(*i)).all(|p| {
        p.runs.len() >= 2
            && p.runs.iter().enumerate().all(|(i, a)| {
                p.runs[i + 1..].iter().all(|b| {
                    crate::math::abs(a.median - b.median)
                        <= a.spread * a.median + b.spread * b.median
                })
            })
    })
}

impl Walk<'_> {
    fn count(c: &Count, at: &str) -> Result<(u64, Basis), PlanRefusal> {
        match c {
            Count::Exact(n) => Ok((*n, Basis::Exact)),
            Count::Expected { value, .. } => Ok((*value, Basis::Expected)),
            Count::WorstCase(n) => Ok((*n, Basis::WorstCase)),
            Count::Unknown(Unknown::Unbounded) => Err(PlanRefusal::at(at, Refusal::UnboundedWork)),
            Count::Unknown(Unknown::DataDependent(w)) => Err(PlanRefusal::at(
                at,
                Refusal::DataDependentUnknown { what: w.clone() },
            )),
        }
    }

    fn leaf(&mut self, l: &Leaf, mult: u64, basis: Basis) -> Result<Interval, PlanRefusal> {
        let at = |r: Refusal| PlanRefusal::at(l.name.clone(), r);
        let (n, lb) = Self::count(&l.count, &l.name)?;
        let curve = selector::select_class(self.evidence, &l.class).map_err(at)?;
        selector::check_implementation(&curve, self.now).map_err(at)?;
        let placement =
            selector::place(&curve, &l.geometry, self.options.allow_bracket).map_err(at)?;
        let per_unit = selector::interval(&curve, &placement, l.geometry.value).map_err(at)?;
        let count = mult.checked_mul(n).ok_or(at(Refusal::Overflow))?;
        let basis = basis.max(lb);
        let mut total = per_unit.times(count).map_err(at)?;
        if basis == Basis::WorstCase {
            total.lower = 0.0;
        }
        let (kind, points) = match placement {
            Placement::Measured { index } => (PlacementKind::Measured, alloc::vec![index]),
            Placement::Interpolated { below } => {
                (PlacementKind::Interpolated, alloc::vec![below, below + 1])
            }
            Placement::Bracketed { below } => {
                (PlacementKind::Bracketed, alloc::vec![below, below + 1])
            }
        };
        let support: Vec<&selector::CurvePoint> =
            points.iter().filter_map(|i| curve.points.get(*i)).collect();
        self.leaves.push(LeafPrediction {
            name: l.name.clone(),
            class: l.class.clone(),
            geometry: l.geometry,
            placement: kind,
            one_regime: kind != PlacementKind::Bracketed,
            points: support.iter().map(|p| p.value).collect(),
            calibrations: support
                .iter()
                .flat_map(|p| p.runs.iter().map(|r| r.calibration.clone()))
                .collect(),
            implementation: curve.implementation.clone(),
            unit: curve.unit.clone(),
            basis,
            count,
            per_unit,
            total,
            runs_agree: agree_all(&curve, &placement),
        });
        Ok(total)
    }

    fn walk(
        &mut self,
        p: &Plan,
        path: &str,
        mult: u64,
        basis: Basis,
    ) -> Result<Interval, PlanRefusal> {
        match p {
            Plan::Leaf(l) => self.leaf(l, mult, basis),
            Plan::Seq(v) => {
                let Some(first) = v.first() else {
                    return Err(PlanRefusal::at(path, Refusal::EmptyPlan));
                };
                if v.len() > 1 {
                    self.note("Seq: interactions between components (cache state left by one for the next) are not modelled");
                }
                let mut acc = self.walk(first, &format!("{path}/0"), mult, basis)?;
                for (i, c) in v.iter().enumerate().skip(1) {
                    let x = self.walk(c, &format!("{path}/{i}"), mult, basis)?;
                    acc = acc.then(&x).map_err(|r| PlanRefusal::at(path, r))?;
                }
                Ok(acc)
            }
            Plan::Repeat { count, body } => {
                let here = format!("{path}/repeat");
                let (n, b) = Self::count(count, &here)?;
                let m = mult
                    .checked_mul(n)
                    .ok_or(PlanRefusal::at(here.clone(), Refusal::Overflow))?;
                self.walk(body, &here, m, basis.max(b))
            }
            Plan::Par {
                branches,
                shared_writes,
            } => {
                let here = format!("{path}/par");
                if branches.is_empty() {
                    return Err(PlanRefusal::at(here, Refusal::EmptyPlan));
                }
                if *shared_writes {
                    return Err(PlanRefusal::at(
                        here,
                        Refusal::UnsupportedConcurrency {
                            reason: ConcurrencyReason::SharedWrites,
                        },
                    ));
                }
                let cores = self
                    .evidence
                    .runs
                    .iter()
                    .map(|r| r.performance_cores)
                    .try_fold(u32::MAX, |acc, c| c.map(|c| acc.min(c)));
                let n = branches.len() as u64;
                if cores.is_none_or(|c| n > u64::from(c)) {
                    return Err(PlanRefusal::at(
                        here,
                        Refusal::UnsupportedConcurrency {
                            reason: ConcurrencyReason::TooManyBranches { branches: n, cores },
                        },
                    ));
                }
                self.has_par = true;
                self.note("Par: elapsed is the critical path of independent branches (never the sum); informational until a holdout supports it");
                let mut items = Vec::with_capacity(branches.len());
                for (i, b) in branches.iter().enumerate() {
                    items.push(self.walk(b, &format!("{here}/{i}"), mult, basis)?);
                }
                Interval::critical_path(&items).ok_or(PlanRefusal::at(here, Refusal::EmptyPlan))
            }
            Plan::Branch(arms) => {
                let here = format!("{path}/branch");
                if arms.is_empty() {
                    return Err(PlanRefusal::at(here, Refusal::EmptyPlan));
                }
                let mut sum: u64 = 0;
                for a in arms {
                    match &a.probability {
                        Some(p) => sum += u64::from(p.per_million),
                        None => return Err(PlanRefusal::at(here, Refusal::UnknownBranch)),
                    }
                }
                if sum != 1_000_000 {
                    return Err(PlanRefusal::at(here, Refusal::UnknownBranch));
                }
                self.has_branch = true;
                self.note("Branch: central is the probability-weighted sum, interval [min lower, max upper]");
                let mut lower = f64::INFINITY;
                let mut upper = f64::NEG_INFINITY;
                let mut central = 0.0;
                for (i, a) in arms.iter().enumerate() {
                    let x = self.walk(&a.plan, &format!("{here}/{i}"), mult, basis)?;
                    let w = a.probability.as_ref().map_or(0, |p| p.per_million);
                    central += f64::from(w) / 1_000_000.0 * x.central;
                    lower = lower.min(x.lower);
                    upper = upper.max(x.upper);
                }
                Ok(Interval {
                    lower,
                    central,
                    upper,
                })
            }
            Plan::Unpriced { name, kind } => Err(PlanRefusal::at(
                name.clone(),
                Refusal::UnpricedComponent { kind: *kind },
            )),
            Plan::Callback { name, callback } => Err(PlanRefusal::at(
                name.clone(),
                Refusal::CallbackUnpriced {
                    callback: callback.clone(),
                },
            )),
            Plan::HistoryDependent { name, what } => Err(PlanRefusal::at(
                name.clone(),
                Refusal::HistoryDependent { what: what.clone() },
            )),
        }
    }

    fn note(&mut self, s: &str) {
        if !self.notes.iter().any(|n| n == s) {
            self.notes.push(s.into());
        }
    }
}

/// Predicts `plan` from `evidence` for the machine and code described by `now`.
///
/// # Errors
///
/// The refusal, naming the leaf (or node) it arose at; `leaf: None` when the evidence as a
/// whole does not apply.
pub fn predict(
    plan: &Plan,
    evidence: &Evidence,
    now: &Now,
    options: Options,
) -> Result<Prediction, PlanRefusal> {
    let applicable = selector::applicability(evidence, now).map_err(PlanRefusal::whole)?;
    let mut w = Walk {
        evidence,
        now,
        options,
        leaves: Vec::new(),
        notes: applicable.notes,
        has_par: false,
        has_branch: false,
    };
    let interval = w.walk(plan, "", 1, Basis::Exact)?;
    let basis = w
        .leaves
        .iter()
        .map(|l| l.basis)
        .max()
        .unwrap_or(Basis::Exact);
    let relative_width = interval.relative_width();
    let admission = evidence.runs.len() >= 2
        && w.leaves.iter().all(|l| {
            l.placement != PlacementKind::Bracketed && l.runs_agree && l.basis == Basis::Exact
        })
        && !w.has_par
        && !w.has_branch
        && relative_width <= ADMISSION_WIDTH;
    if basis == Basis::WorstCase {
        w.note("WorstCase: the interval is an upper bound (lower = 0)");
    }
    Ok(Prediction {
        method: METHOD.into(),
        revision: options.revision().into(),
        plan_digest: plan.digest(),
        interval,
        relative_width,
        grade: if admission {
            Grade::Admission
        } else {
            Grade::Informational
        },
        basis,
        leaves: w.leaves,
        runs: evidence
            .runs
            .iter()
            .map(|r| RunRef {
                identity_digest: r.identity_digest.clone(),
                environment_digest: r.environment_digest.clone(),
                revision: r.revision.clone(),
                method_digest: r.method_digest.clone(),
                conditions: r.conditions.clone(),
            })
            .collect(),
        notes: w.notes,
    })
}

/// One result per path of a plan with branches (a single prediction of a plan whose branches
/// have no probabilities is refused with `UnknownBranch`).
pub fn predict_paths(
    plan: &Plan,
    evidence: &Evidence,
    now: &Now,
    options: Options,
) -> Vec<PathPrediction> {
    plan.paths()
        .into_iter()
        .map(|(choices, p)| PathPrediction {
            choices,
            result: predict(&p, evidence, now, options),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::class::Ordering;
    use crate::geometry::Parameter;
    use crate::plan::{Arm, Probability};
    use crate::refusal::{StaleReason, UnpricedKind};
    use crate::selector::tests::{atomic, cal, now_for, run, transform, transform_curve};
    use alloc::boxed::Box;
    use alloc::vec;

    fn evidence(runs: usize) -> Evidence {
        let a = atomic(Ordering::AcqRel);
        let mut cals = transform_curve(256);
        cals.push(cal(&a, Parameter::Ops, 1 << 20, 1.74, 0.02));
        cals.push(cal(&a, Parameter::Ops, 1 << 22, 1.75, 0.02));
        Evidence {
            runs: (0..runs).map(|_| run(cals.clone())).collect(),
        }
    }

    fn now() -> Now {
        now_for(&[transform(256), atomic(Ordering::AcqRel)])
    }

    fn tleaf(name: &str, extent: u64, count: Count) -> Plan {
        Plan::Leaf(Leaf {
            name: name.into(),
            class: transform(256),
            geometry: Geometry::new(Parameter::ExtentBytes, extent).unwrap(),
            count,
        })
    }

    fn aleaf(name: &str, ordering: Ordering, ops: u64) -> Plan {
        Plan::Leaf(Leaf {
            name: name.into(),
            class: atomic(ordering),
            geometry: Geometry::new(Parameter::Ops, ops).unwrap(),
            count: Count::Exact(ops),
        })
    }

    #[test]
    fn one_leaf_measured_is_admission() {
        let p = tleaf("t", 262_144, Count::Exact(262_144));
        let r = predict(&p, &evidence(2), &now(), Options::CLASS_A).unwrap();
        assert_eq!(r.grade, Grade::Admission);
        assert_eq!(r.leaves[0].placement, PlacementKind::Measured);
        assert_eq!(r.revision, "A");
        assert_eq!(r.plan_digest, p.digest());
        assert!((r.interval.central - 0.019 * 262_144.0).abs() < 1e-9);
        // One run is never admission.
        let r1 = predict(&p, &evidence(1), &now(), Options::CLASS_A).unwrap();
        assert_eq!(r1.grade, Grade::Informational);
    }

    #[test]
    fn seq_adds_and_names_interactions() {
        let p = Plan::Seq(vec![
            tleaf("t", 262_144, Count::Exact(262_144)),
            aleaf("a", Ordering::AcqRel, 1 << 20),
        ]);
        let r = predict(&p, &evidence(2), &now(), Options::CLASS_A).unwrap();
        let sum = r.leaves[0].total.central + r.leaves[1].total.central;
        assert!((r.interval.central - sum).abs() < 1e-9);
        assert!(r.notes.iter().any(|n| n.contains("not modelled")));
        assert_eq!(r.leaves.len(), 2);
    }

    #[test]
    fn repeat_multiplies_and_overflow_is_refused() {
        let body = tleaf("t", 262_144, Count::Exact(262_144));
        let p = Plan::Repeat {
            count: Count::Exact(4),
            body: Box::new(body.clone()),
        };
        let r = predict(&p, &evidence(2), &now(), Options::CLASS_A).unwrap();
        assert_eq!(r.leaves[0].count, 4 * 262_144);
        let p = Plan::Repeat {
            count: Count::Exact(u64::MAX),
            body: Box::new(body),
        };
        let e = predict(&p, &evidence(2), &now(), Options::CLASS_A).unwrap_err();
        assert_eq!(e.refusal, Refusal::Overflow);
        assert_eq!(e.leaf.as_deref(), Some("t"));
        let p = Plan::Repeat {
            count: Count::Exact(u64::MAX),
            body: Box::new(Plan::Repeat {
                count: Count::Exact(2),
                body: Box::new(tleaf("t", 262_144, Count::Exact(1))),
            }),
        };
        let e = predict(&p, &evidence(2), &now(), Options::CLASS_A).unwrap_err();
        assert_eq!(e.refusal, Refusal::Overflow);
        assert_eq!(e.leaf.as_deref(), Some("/repeat/repeat"));
    }

    #[test]
    fn exact_expected_and_worst_case_counts() {
        let exact = predict(
            &tleaf("t", 262_144, Count::Exact(1000)),
            &evidence(2),
            &now(),
            Options::CLASS_A,
        )
        .unwrap();
        let worst = predict(
            &tleaf("t", 262_144, Count::WorstCase(1000)),
            &evidence(2),
            &now(),
            Options::CLASS_A,
        )
        .unwrap();
        assert_eq!(worst.basis, Basis::WorstCase);
        assert_eq!(worst.interval.lower, 0.0);
        assert_eq!(worst.interval.upper, exact.interval.upper);
        assert_eq!(worst.grade, Grade::Informational);
        let expected = predict(
            &tleaf(
                "t",
                262_144,
                Count::Expected {
                    value: 1000,
                    evidence: "test".into(),
                },
            ),
            &evidence(2),
            &now(),
            Options::CLASS_A,
        )
        .unwrap();
        assert_eq!(expected.basis, Basis::Expected);
        assert_eq!(expected.grade, Grade::Informational);
        // A worst-case repeat makes every leaf below it a bound.
        let p = Plan::Repeat {
            count: Count::WorstCase(3),
            body: Box::new(tleaf("t", 262_144, Count::Exact(1000))),
        };
        let r = predict(&p, &evidence(2), &now(), Options::CLASS_A).unwrap();
        assert_eq!((r.basis, r.interval.lower), (Basis::WorstCase, 0.0));
        let u = predict(
            &tleaf("t", 262_144, Count::Unknown(Unknown::Unbounded)),
            &evidence(2),
            &now(),
            Options::CLASS_A,
        )
        .unwrap_err();
        assert_eq!(u.refusal, Refusal::UnboundedWork);
        let d = predict(
            &tleaf(
                "t",
                262_144,
                Count::Unknown(Unknown::DataDependent("rows".into())),
            ),
            &evidence(2),
            &now(),
            Options::CLASS_A,
        )
        .unwrap_err();
        assert_eq!(d.refusal.variant(), "DataDependentUnknown");
        let r = predict(
            &Plan::Repeat {
                count: Count::Unknown(Unknown::Unbounded),
                body: Box::new(tleaf("t", 262_144, Count::Exact(1))),
            },
            &evidence(2),
            &now(),
            Options::CLASS_A,
        )
        .unwrap_err();
        assert_eq!(
            (r.leaf.as_deref(), r.refusal),
            (Some("/repeat"), Refusal::UnboundedWork)
        );
    }

    #[test]
    fn par_is_the_critical_path_and_its_rules() {
        let a = tleaf("a", 262_144, Count::Exact(262_144));
        let b = tleaf("b", 1_048_576, Count::Exact(1_048_576));
        let p = Plan::Par {
            branches: vec![a.clone(), b.clone()],
            shared_writes: false,
        };
        let r = predict(&p, &evidence(2), &now(), Options::CLASS_A).unwrap();
        assert_eq!(r.interval, r.leaves[1].total);
        assert_eq!(r.grade, Grade::Informational);
        let shared = Plan::Par {
            branches: vec![a.clone(), b.clone()],
            shared_writes: true,
        };
        let e = predict(&shared, &evidence(2), &now(), Options::CLASS_A).unwrap_err();
        assert_eq!(
            e.refusal,
            Refusal::UnsupportedConcurrency {
                reason: ConcurrencyReason::SharedWrites
            }
        );
        let many = Plan::Par {
            branches: (0..11).map(|_| a.clone()).collect(),
            shared_writes: false,
        };
        let e = predict(&many, &evidence(2), &now(), Options::CLASS_A).unwrap_err();
        assert_eq!(
            e.refusal,
            Refusal::UnsupportedConcurrency {
                reason: ConcurrencyReason::TooManyBranches {
                    branches: 11,
                    cores: Some(10)
                }
            }
        );
        let mut unknown_cores = evidence(2);
        unknown_cores.runs[0].performance_cores = None;
        let e = predict(&p, &unknown_cores, &now(), Options::CLASS_A).unwrap_err();
        assert!(matches!(
            e.refusal,
            Refusal::UnsupportedConcurrency {
                reason: ConcurrencyReason::TooManyBranches { cores: None, .. }
            }
        ));
    }

    #[test]
    fn branches() {
        let arm = |p: Option<u32>, plan: Plan| Arm {
            probability: p.map(|per_million| Probability {
                per_million,
                evidence: "test".into(),
            }),
            plan,
        };
        let a = tleaf("a", 262_144, Count::Exact(1000));
        let b = tleaf("b", 262_144, Count::Exact(3000));
        let no = Plan::Branch(vec![arm(None, a.clone()), arm(None, b.clone())]);
        let e = predict(&no, &evidence(2), &now(), Options::CLASS_A).unwrap_err();
        assert_eq!(e.refusal, Refusal::UnknownBranch);
        let paths = predict_paths(&no, &evidence(2), &now(), Options::CLASS_A);
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().all(|p| p.result.is_ok()));
        let bad = Plan::Branch(vec![
            arm(Some(500_000), a.clone()),
            arm(Some(400_000), b.clone()),
        ]);
        assert_eq!(
            predict(&bad, &evidence(2), &now(), Options::CLASS_A)
                .unwrap_err()
                .refusal,
            Refusal::UnknownBranch
        );
        let ok = Plan::Branch(vec![arm(Some(750_000), a), arm(Some(250_000), b)]);
        let r = predict(&ok, &evidence(2), &now(), Options::CLASS_A).unwrap();
        let (x, y) = (r.leaves[0].total, r.leaves[1].total);
        assert!((r.interval.central - (0.75 * x.central + 0.25 * y.central)).abs() < 1e-9);
        assert_eq!(r.interval.lower, x.lower);
        assert_eq!(r.interval.upper, y.upper);
        assert_eq!(r.grade, Grade::Informational);
    }

    #[test]
    fn refused_leaves_name_themselves() {
        let e = evidence(2);
        let n = now();
        let p = Plan::Seq(vec![
            tleaf("ok", 262_144, Count::Exact(1)),
            Plan::Unpriced {
                name: "syscall".into(),
                kind: UnpricedKind::KernelEntry,
            },
        ]);
        let r = predict(&p, &e, &n, Options::CLASS_A).unwrap_err();
        assert_eq!(r.leaf.as_deref(), Some("syscall"));
        assert_eq!(
            r.refusal,
            Refusal::UnpricedComponent {
                kind: UnpricedKind::KernelEntry
            }
        );
        let p = Plan::Callback {
            name: "visit".into(),
            callback: "user closure".into(),
        };
        assert_eq!(
            predict(&p, &e, &n, Options::CLASS_A).unwrap_err().refusal,
            Refusal::CallbackUnpriced {
                callback: "user closure".into()
            }
        );
        let p = Plan::HistoryDependent {
            name: "third read".into(),
            what: "pages already held".into(),
        };
        assert_eq!(
            predict(&p, &e, &n, Options::CLASS_A)
                .unwrap_err()
                .refusal
                .variant(),
            "HistoryDependent"
        );
        for empty in [
            Plan::Seq(vec![]),
            Plan::Par {
                branches: vec![],
                shared_writes: false,
            },
            Plan::Branch(vec![]),
        ] {
            assert_eq!(
                predict(&empty, &e, &n, Options::CLASS_A)
                    .unwrap_err()
                    .refusal,
                Refusal::EmptyPlan
            );
        }
    }

    #[test]
    fn missing_calibration_wrong_block_ordering_and_range() {
        let e = evidence(2);
        let n = now();
        let p = Plan::Leaf(Leaf {
            name: "t64".into(),
            class: transform(64),
            geometry: Geometry::new(Parameter::ExtentBytes, 16384).unwrap(),
            count: Count::Exact(16384),
        });
        assert_eq!(
            predict(&p, &e, &n, Options::CLASS_B)
                .unwrap_err()
                .refusal
                .variant(),
            "ClassMismatch"
        );
        let p = aleaf("rel", Ordering::Release, 1 << 21);
        assert_eq!(
            predict(&p, &e, &n, Options::CLASS_B)
                .unwrap_err()
                .refusal
                .variant(),
            "UnsupportedOrdering"
        );
        let p = tleaf("big", 1 << 26, Count::Exact(1));
        assert_eq!(
            predict(&p, &e, &n, Options::CLASS_B)
                .unwrap_err()
                .refusal
                .variant(),
            "OutOfRange"
        );
        let p = tleaf("boundary", 131_072, Count::Exact(1));
        assert_eq!(
            predict(&p, &e, &n, Options::CLASS_A)
                .unwrap_err()
                .refusal
                .variant(),
            "CrossesRegimeBoundary"
        );
        let b = predict(&p, &e, &n, Options::CLASS_B).unwrap();
        assert_eq!(b.leaves[0].placement, PlacementKind::Bracketed);
        assert_eq!(b.grade, Grade::Informational);
        assert_eq!(b.revision, "B");
        let p = Plan::Leaf(Leaf {
            name: "geom".into(),
            class: transform(256),
            geometry: Geometry::new(Parameter::Items, 16384).unwrap(),
            count: Count::Exact(1),
        });
        assert_eq!(
            predict(&p, &e, &n, Options::CLASS_A)
                .unwrap_err()
                .refusal
                .variant(),
            "MissingGeometry"
        );
    }

    #[test]
    fn stale_and_wrong_machine_refuse_the_whole_plan() {
        let p = tleaf("t", 262_144, Count::Exact(1));
        let mut n = now();
        n.identity_digest = "sha256:another-machine".into();
        let r = predict(&p, &evidence(2), &n, Options::CLASS_A).unwrap_err();
        assert_eq!((r.leaf, r.refusal.variant()), (None, "MachineMismatch"));
        let mut n = now();
        n.build.rustflags = "-C target-cpu=native".into();
        let r = predict(&p, &evidence(2), &n, Options::CLASS_A).unwrap_err();
        assert!(matches!(
            r.refusal,
            Refusal::StaleProfile {
                reason: StaleReason::Build { .. }
            }
        ));
        let mut n = now();
        n.implementations[0].implementation.source_digest = "sha256:edited".into();
        let r = predict(&p, &evidence(2), &n, Options::CLASS_A).unwrap_err();
        assert_eq!(r.refusal.variant(), "ImplementationStale");
        assert_eq!(r.leaf.as_deref(), Some("t"));
    }

    #[test]
    fn wide_interval_is_informational() {
        let mut e = evidence(2);
        for r in &mut e.runs {
            r.calibrations[2].spread = 0.15;
        }
        let p = tleaf("t", 262_144, Count::Exact(1));
        let r = predict(&p, &e, &now(), Options::CLASS_A).unwrap();
        assert!(r.relative_width > ADMISSION_WIDTH);
        assert_eq!(r.grade, Grade::Informational);
    }

    #[test]
    fn disagreeing_runs_are_informational() {
        let mut e = evidence(2);
        e.runs[1].calibrations[2].median = 0.0250;
        e.runs[1].calibrations[2].round_low = 0.0249;
        e.runs[1].calibrations[2].round_high = 0.0251;
        let p = tleaf("t", 262_144, Count::Exact(1));
        let r = predict(&p, &e, &now(), Options::CLASS_A).unwrap();
        assert!(!r.leaves[0].runs_agree);
        assert_eq!(r.grade, Grade::Informational);
        // The interval holds both runs' round medians.
        assert!(r.interval.lower <= 0.019 && r.interval.upper >= 0.0251);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip() {
        let p = Plan::Seq(vec![
            tleaf("t", 262_144, Count::Exact(1)),
            Plan::Repeat {
                count: Count::WorstCase(2),
                body: Box::new(aleaf("a", Ordering::AcqRel, 1 << 20)),
            },
        ]);
        let text = serde_json::to_string(&p).unwrap();
        let back: Plan = serde_json::from_str(&text).unwrap();
        assert_eq!(back, p);
        let e = evidence(2);
        let et = serde_json::to_string(&e).unwrap();
        assert_eq!(serde_json::from_str::<Evidence>(&et).unwrap(), e);
        let n = now();
        let nt = serde_json::to_string(&n).unwrap();
        assert_eq!(serde_json::from_str::<Now>(&nt).unwrap(), n);
        let r = predict(&p, &e, &n, Options::CLASS_A).unwrap();
        let rt = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<Prediction>(&rt).unwrap(), r);
        let refusal = predict(
            &aleaf("x", Ordering::SeqCst, 1 << 21),
            &e,
            &n,
            Options::CLASS_A,
        )
        .unwrap_err();
        let ft = serde_json::to_string(&refusal).unwrap();
        assert_eq!(serde_json::from_str::<PlanRefusal>(&ft).unwrap(), refusal);
        // Invalid values are refused while reading.
        assert!(serde_json::from_str::<Geometry>(r#"{"parameter":"ops","value":0}"#).is_err());
        assert!(
            serde_json::from_str::<crate::class::ClassKey>(
                r#"[{"block_bytes":64},{"block_bytes":256}]"#
            )
            .is_err()
        );
    }
}
