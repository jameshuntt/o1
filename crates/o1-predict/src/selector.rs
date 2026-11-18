//! Profile and calibration selection (design §13), in four separate steps, each returning a
//! value or a refusal:
//!
//! 1. [`applicability`]: machine, schema, method, provenance (§14).
//! 2. [`select_class`]: the exact class, or the refusal naming the differing part.
//! 3. [`place`]: `Measured` / `Interpolated` / `Bracketed`, or out of range.
//! 4. [`interval`]: the §11 interval at the placed geometry.
//!
//! [`check_implementation`] (the measured code against the caller's now) sits between 2 and 3.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::class::OperationClass;
use crate::evidence::{CalibrationStatus, Evidence, Now};
use crate::geometry::{Geometry, Parameter};
use crate::identity::Implementation;
use crate::interval::{Interval, RunPoint};
use crate::math;
use crate::refusal::{Refusal, StaleReason};

/// Step 1's result: the evidence applies; what was reported but not refused on.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Applicable {
    /// Runs supplied.
    pub runs: usize,
    /// Reported facts that are not refusals (environment changed, revisions).
    pub notes: Vec<String>,
}

fn stale(reason: StaleReason) -> Refusal {
    Refusal::StaleProfile { reason }
}

/// Step 1: do the runs apply to this machine and this code? Refuses, in this order: no run or
/// malformed evidence ([`Refusal::StaleProfile`]); runs that are not one machine, schema,
/// method or build ([`Refusal::ProfilesDisagreeOnIdentity`]); another machine
/// ([`Refusal::MachineMismatch`]); another schema, unclean provenance, a binary not built
/// from its source, another build ([`Refusal::StaleProfile`]); another method
/// ([`Refusal::MethodStale`]). An environment (OS, kernel) change is reported, not refused
/// (§14).
///
/// # Errors
///
/// The refusal.
pub fn applicability(evidence: &Evidence, now: &Now) -> Result<Applicable, Refusal> {
    let Some(first) = evidence.runs.first() else {
        return Err(stale(StaleReason::NoProfile));
    };
    if let Err(e) = evidence.validate() {
        return Err(stale(StaleReason::Malformed {
            what: format!("{e}"),
        }));
    }
    for r in &evidence.runs[1..] {
        let what = if r.identity_digest != first.identity_digest {
            Some("identity digest")
        } else if r.schema != first.schema {
            Some("schema")
        } else if r.method_digest != first.method_digest {
            Some("method digest")
        } else if r.build != first.build {
            Some("build")
        } else {
            None
        };
        if let Some(what) = what {
            return Err(Refusal::ProfilesDisagreeOnIdentity { what: what.into() });
        }
    }
    if first.identity_digest != now.identity_digest {
        return Err(Refusal::MachineMismatch {
            profile: first.identity_digest.clone(),
            now: now.identity_digest.clone(),
        });
    }
    if first.schema != now.schema {
        return Err(stale(StaleReason::Schema {
            profile: first.schema.clone(),
            now: now.schema.clone(),
        }));
    }
    for r in &evidence.runs {
        if !r.clean {
            return Err(stale(StaleReason::Unclean));
        }
        if !r.built_from_source {
            return Err(stale(StaleReason::NotBuiltFromSource));
        }
    }
    let b = &first.build;
    let n = &now.build;
    for (what, p, q) in [
        ("toolchain", &b.toolchain, &n.toolchain),
        ("target", &b.target, &n.target),
        ("cargo_profile", &b.cargo_profile, &n.cargo_profile),
        ("opt_level", &b.opt_level, &n.opt_level),
        ("rustflags", &b.rustflags, &n.rustflags),
    ] {
        if p != q {
            return Err(stale(StaleReason::Build {
                what: what.into(),
                profile: p.clone(),
                now: q.clone(),
            }));
        }
    }
    if first.method_digest != now.method_digest {
        return Err(Refusal::MethodStale {
            profile: first.method_digest.clone(),
            now: now.method_digest.clone(),
        });
    }
    let mut notes = Vec::new();
    for (i, r) in evidence.runs.iter().enumerate() {
        if r.environment_digest != now.environment_digest {
            notes.push(format!(
                "run {i}: environment (OS / kernel) differs from now; reported, not refused"
            ));
        }
    }
    Ok(Applicable {
        runs: evidence.runs.len(),
        notes,
    })
}

/// One point of a class's curve: its value and each run's numbers there.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CurvePoint {
    /// Parameter value.
    pub value: u64,
    /// One entry per run, in run order.
    pub runs: Vec<RunPoint>,
}

impl CurvePoint {
    /// Median over the runs of the pooled medians.
    pub fn central(&self) -> f64 {
        let m: Vec<f64> = self.runs.iter().map(|r| r.median).collect();
        crate::interval::median(&m)
    }
}

/// A class's measured curve over every run supplied.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Curve {
    /// The class.
    pub class: OperationClass,
    /// The parameter.
    pub parameter: Parameter,
    /// Unit of work.
    pub unit: String,
    /// The code measured.
    pub implementation: Implementation,
    /// Points, ascending (only values present in every run).
    pub points: Vec<CurvePoint>,
}

/// Step 2: the curve of exactly `class`. When no calibration has the class, the calibrations
/// of the same operation id are compared with it and the nearest (fewest differing parts) names
/// the refusal ([`Refusal::for_differing`]); no calibration of the id at all is
/// [`Refusal::NoCalibration`]. The runs must agree on the class's implementation
/// ([`Refusal::ProfilesDisagreeOnIdentity`]); a value measured in some runs only is not a point.
///
/// # Errors
///
/// The refusal.
pub fn select_class(evidence: &Evidence, class: &OperationClass) -> Result<Curve, Refusal> {
    let mut per_run: Vec<Vec<&crate::evidence::CalibrationEvidence>> = Vec::new();
    for run in &evidence.runs {
        per_run.push(
            run.calibrations
                .iter()
                .filter(|c| &c.class == class)
                .collect(),
        );
    }
    if per_run.iter().any(Vec::is_empty) {
        let mut best: Option<Vec<crate::class::PartKind>> = None;
        for run in &evidence.runs {
            for c in run.calibrations.iter().filter(|c| c.class.id == class.id) {
                let d = class.key.differing(&c.class.key);
                if d.is_empty() {
                    continue;
                }
                let better = match &best {
                    None => true,
                    Some(b) => d.len() < b.len() || (d.len() == b.len() && d < *b),
                };
                if better {
                    best = Some(d);
                }
            }
        }
        return Err(match best {
            Some(parts) => Refusal::for_differing(class, parts),
            None => Refusal::NoCalibration {
                class: class.clone(),
            },
        });
    }
    let first = per_run[0][0];
    for cs in &per_run {
        for c in cs {
            if c.implementation != first.implementation {
                return Err(Refusal::ProfilesDisagreeOnIdentity {
                    what: format!("implementation of {}", c.id),
                });
            }
            if c.parameter != first.parameter || c.unit != first.unit {
                return Err(Refusal::ProfilesDisagreeOnIdentity {
                    what: format!("parameter or unit of {}", c.id),
                });
            }
        }
    }
    let mut values: Vec<u64> = per_run[0].iter().map(|c| c.value).collect();
    values.sort_unstable();
    values.retain(|v| per_run.iter().all(|cs| cs.iter().any(|c| c.value == *v)));
    let points = values
        .into_iter()
        .map(|v| CurvePoint {
            value: v,
            runs: per_run
                .iter()
                .enumerate()
                .filter_map(|(r, cs)| {
                    cs.iter().find(|c| c.value == v).map(|c| RunPoint {
                        run: r,
                        calibration: c.id.clone(),
                        median: c.median,
                        spread: c.spread,
                        round_low: c.round_low,
                        round_high: c.round_high,
                        stable: c.status == CalibrationStatus::Stable,
                    })
                })
                .collect(),
        })
        .collect();
    Ok(Curve {
        class: class.clone(),
        parameter: first.parameter,
        unit: first.unit.clone(),
        implementation: first.implementation.clone(),
        points,
    })
}

/// Between 2 and 3: the measured code must be the code now.
///
/// # Errors
///
/// [`Refusal::ImplementationStale`].
pub fn check_implementation(curve: &Curve, now: &Now) -> Result<(), Refusal> {
    let current = now.implementation(&curve.class.id);
    if current == Some(&curve.implementation) {
        Ok(())
    } else {
        Err(Refusal::ImplementationStale {
            class: curve.class.clone(),
            measured: curve.implementation.clone(),
            now: current.cloned(),
        })
    }
}

/// Where a geometry lies on a curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Placement {
    /// At a measured point (its index).
    Measured {
        /// Index into the curve's points.
        index: usize,
    },
    /// Strictly between adjacent points that are one regime.
    Interpolated {
        /// Index of the point below.
        below: usize,
    },
    /// Strictly between adjacent points that are not one regime (only when allowed).
    Bracketed {
        /// Index of the point below.
        below: usize,
    },
}

/// Unit 4's repeatability rule between two adjacent points, in every run supplied:
/// `|M0 − M1| ≤ s0·M0 + s1·M1`.
pub fn one_regime(a: &CurvePoint, b: &CurvePoint) -> bool {
    a.runs
        .iter()
        .zip(&b.runs)
        .all(|(x, y)| math::abs(x.median - y.median) <= x.spread * x.median + y.spread * y.median)
        && a.runs.len() == b.runs.len()
}

/// Step 3: place `geometry` on `curve`. No extrapolation: outside `[min, max]` is always
/// refused; a single-point curve supports only its point.
///
/// # Errors
///
/// [`Refusal::MissingGeometry`], [`Refusal::OutOfRange`], [`Refusal::CrossesRegimeBoundary`].
pub fn place(
    curve: &Curve,
    geometry: &Geometry,
    allow_bracket: bool,
) -> Result<Placement, Refusal> {
    if geometry.parameter != curve.parameter {
        return Err(Refusal::MissingGeometry {
            expected: curve.parameter,
            given: geometry.parameter,
        });
    }
    let x = geometry.value;
    let (Some(lo), Some(hi)) = (curve.points.first(), curve.points.last()) else {
        return Err(Refusal::NoCalibration {
            class: curve.class.clone(),
        });
    };
    if x < lo.value || x > hi.value {
        return Err(Refusal::OutOfRange {
            parameter: curve.parameter,
            value: x,
            min: lo.value,
            max: hi.value,
        });
    }
    if let Some(index) = curve.points.iter().position(|p| p.value == x) {
        return Ok(Placement::Measured { index });
    }
    let below = curve.points.iter().rposition(|p| p.value < x).unwrap_or(0);
    let (a, b) = (&curve.points[below], &curve.points[below + 1]);
    if one_regime(a, b) {
        Ok(Placement::Interpolated { below })
    } else if allow_bracket {
        Ok(Placement::Bracketed { below })
    } else {
        Err(Refusal::CrossesRegimeBoundary {
            parameter: curve.parameter,
            value: x,
            below: a.value,
            above: b.value,
        })
    }
}

/// Step 4: the per-unit interval at `x` for `placement` (§7, §11). Measured: the point's
/// interval. Interpolated: central linear in `log2(x)` between the two points' centrals,
/// interval the hull of both. Bracketed: the hull, central the geometric mean. Every
/// supporting point must be stable in every run.
///
/// # Errors
///
/// [`Refusal::UnstableCalibration`].
pub fn interval(curve: &Curve, placement: &Placement, x: u64) -> Result<Interval, Refusal> {
    let support: Vec<&CurvePoint> = match *placement {
        Placement::Measured { index } => curve.points.get(index).into_iter().collect(),
        Placement::Interpolated { below } | Placement::Bracketed { below } => curve
            .points
            .get(below..below + 2)
            .map(|s| s.iter().collect())
            .unwrap_or_default(),
    };
    if support.is_empty() {
        return Err(Refusal::NoCalibration {
            class: curve.class.clone(),
        });
    }
    for p in &support {
        if let Some(r) = p.runs.iter().find(|r| !r.stable) {
            return Err(Refusal::UnstableCalibration {
                calibration: r.calibration.clone(),
                run: r.run,
            });
        }
    }
    let ivs: Vec<Interval> = support
        .iter()
        .filter_map(|p| Interval::of_point(&p.runs))
        .collect();
    match (placement, ivs.as_slice()) {
        (Placement::Measured { .. }, [i]) => Ok(*i),
        (Placement::Interpolated { .. }, [a, b]) => {
            let (x0, x1) = (support[0].value, support[1].value);
            let t = (math::log2_u64(x) - math::log2_u64(x0))
                / (math::log2_u64(x1) - math::log2_u64(x0));
            let central = a.central + t * (b.central - a.central);
            Ok(Interval::hull(a, b, central))
        }
        (Placement::Bracketed { .. }, [a, b]) => {
            Ok(Interval::hull(a, b, math::sqrt(a.central * b.central)))
        }
        _ => Err(Refusal::NoCalibration {
            class: curve.class.clone(),
        }),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::class::{AtomicOp, ClassKey, KeyPart, Ordering, Sharing};
    use crate::evidence::{
        BuildFacts, CalibrationEvidence, CurrentImplementation, ProfileEvidence,
    };
    use crate::identity::OperationId;
    use alloc::vec;

    pub(crate) fn transform(block: u32) -> OperationClass {
        OperationClass::new(
            OperationId::new("o1-core", "stream::BlockPass::transform"),
            ClassKey::new(vec![KeyPart::BlockBytes(block)]).unwrap(),
        )
    }

    pub(crate) fn atomic(ordering: Ordering) -> OperationClass {
        OperationClass::new(
            OperationId::new("o1-machine-profile", "kernels::atomics::fetch_add"),
            ClassKey::new(vec![
                KeyPart::Atomic {
                    op: AtomicOp::Rmw,
                    ordering,
                },
                KeyPart::Sharing(Sharing::Exclusive),
            ])
            .unwrap(),
        )
    }

    pub(crate) fn imp() -> Implementation {
        Implementation::new("0.0.0", "sha256:aa")
    }

    pub(crate) fn cal(
        class: &OperationClass,
        param: Parameter,
        value: u64,
        median: f64,
        spread: f64,
    ) -> CalibrationEvidence {
        CalibrationEvidence {
            id: format!("{}.{value}", class.id.path),
            class: class.clone(),
            implementation: imp(),
            parameter: param,
            value,
            unit: "byte".into(),
            median,
            spread,
            round_low: median * (1.0 - spread / 2.0),
            round_high: median * (1.0 + spread / 2.0),
            status: CalibrationStatus::Stable,
            conditions: "test".into(),
        }
    }

    pub(crate) fn run(calibrations: Vec<CalibrationEvidence>) -> ProfileEvidence {
        ProfileEvidence {
            schema: "o1.machine-profile.v2".into(),
            identity_digest: "sha256:machine".into(),
            environment_digest: "sha256:env".into(),
            method_digest: "sha256:method".into(),
            revision: "r".into(),
            clean: true,
            built_from_source: true,
            build: BuildFacts {
                toolchain: "rustc 1".into(),
                target: "t".into(),
                cargo_profile: "release".into(),
                opt_level: "3".into(),
                rustflags: String::new(),
            },
            performance_cores: Some(10),
            conditions: "test".into(),
            calibrations,
        }
    }

    pub(crate) fn now_for(classes: &[OperationClass]) -> Now {
        Now {
            schema: "o1.machine-profile.v2".into(),
            identity_digest: "sha256:machine".into(),
            environment_digest: "sha256:env".into(),
            method_digest: "sha256:method".into(),
            build: run(vec![]).build,
            implementations: classes
                .iter()
                .map(|c| CurrentImplementation {
                    id: c.id.clone(),
                    implementation: imp(),
                })
                .collect(),
        }
    }

    /// A transform curve with a regime boundary between 65536 and 262144 (×2 per byte).
    pub(crate) fn transform_curve(block: u32) -> Vec<CalibrationEvidence> {
        let c = transform(block);
        vec![
            cal(&c, Parameter::ExtentBytes, 16384, 0.0086, 0.01),
            cal(&c, Parameter::ExtentBytes, 65536, 0.0088, 0.03),
            cal(&c, Parameter::ExtentBytes, 262_144, 0.0190, 0.01),
            cal(&c, Parameter::ExtentBytes, 1_048_576, 0.0191, 0.01),
        ]
    }

    fn evidence() -> Evidence {
        Evidence {
            runs: vec![run(transform_curve(256)), run(transform_curve(256))],
        }
    }

    #[test]
    fn applicability_every_refusal() {
        let e = evidence();
        let now = now_for(&[transform(256)]);
        assert!(applicability(&e, &now).is_ok());
        assert_eq!(
            applicability(&Evidence::default(), &now),
            Err(Refusal::StaleProfile {
                reason: StaleReason::NoProfile
            })
        );
        let mut n = now.clone();
        n.identity_digest = "sha256:other".into();
        assert_eq!(
            applicability(&e, &n).unwrap_err().variant(),
            "MachineMismatch"
        );
        let mut n = now.clone();
        n.schema = "o1.machine-profile.v1".into();
        assert!(matches!(
            applicability(&e, &n),
            Err(Refusal::StaleProfile {
                reason: StaleReason::Schema { .. }
            })
        ));
        let mut n = now.clone();
        n.build.toolchain = "rustc 2".into();
        assert!(matches!(
            applicability(&e, &n),
            Err(Refusal::StaleProfile {
                reason: StaleReason::Build { .. }
            })
        ));
        let mut n = now.clone();
        n.method_digest = "sha256:new-method".into();
        assert_eq!(applicability(&e, &n).unwrap_err().variant(), "MethodStale");
        let mut u = e.clone();
        u.runs[1].clean = false;
        assert_eq!(
            applicability(&u, &now),
            Err(Refusal::StaleProfile {
                reason: StaleReason::Unclean
            })
        );
        let mut u = e.clone();
        u.runs[0].built_from_source = false;
        assert_eq!(
            applicability(&u, &now),
            Err(Refusal::StaleProfile {
                reason: StaleReason::NotBuiltFromSource
            })
        );
        let mut d = e.clone();
        d.runs[1].identity_digest = "sha256:x".into();
        assert_eq!(
            applicability(&d, &now).unwrap_err().variant(),
            "ProfilesDisagreeOnIdentity"
        );
        let mut d = e.clone();
        d.runs[1].method_digest = "sha256:x".into();
        assert_eq!(
            applicability(&d, &now).unwrap_err().variant(),
            "ProfilesDisagreeOnIdentity"
        );
        let mut m = e.clone();
        m.runs[0].calibrations[0].median = f64::NAN;
        assert!(matches!(
            applicability(&m, &now),
            Err(Refusal::StaleProfile {
                reason: StaleReason::Malformed { .. }
            })
        ));
        // Environment: reported, not refused.
        let mut n = now.clone();
        n.environment_digest = "sha256:other-os".into();
        let a = applicability(&e, &n).unwrap();
        assert_eq!(a.notes.len(), 2);
    }

    #[test]
    fn select_exact_class_and_name_the_differing_part() {
        let e = evidence();
        let c = select_class(&e, &transform(256)).unwrap();
        assert_eq!(c.points.len(), 4);
        assert_eq!(c.points[0].runs.len(), 2);
        let r = select_class(&e, &transform(128)).unwrap_err();
        assert_eq!(
            r,
            Refusal::ClassMismatch {
                class: transform(128),
                parts: vec![crate::class::PartKind::BlockBytes]
            }
        );
        let other = OperationClass::new(
            OperationId::new("o1-core", "stream::BlockPass::read"),
            ClassKey::new(vec![KeyPart::BlockBytes(256)]).unwrap(),
        );
        assert_eq!(
            select_class(&e, &other).unwrap_err().variant(),
            "NoCalibration"
        );
    }

    #[test]
    fn select_refuses_unsupported_ordering() {
        let calibrated = atomic(Ordering::AcqRel);
        let e = Evidence {
            runs: vec![run(vec![
                cal(&calibrated, Parameter::Ops, 1 << 20, 1.74, 0.02),
                cal(&calibrated, Parameter::Ops, 1 << 22, 1.75, 0.02),
            ])],
        };
        for o in [Ordering::Release, Ordering::SeqCst, Ordering::Acquire] {
            assert_eq!(
                select_class(&e, &atomic(o)).unwrap_err().variant(),
                "UnsupportedOrdering"
            );
        }
        // A compare-exchange is not an RMW.
        let cas = OperationClass::new(
            calibrated.id.clone(),
            ClassKey::new(vec![
                KeyPart::Atomic {
                    op: AtomicOp::CompareExchange,
                    ordering: Ordering::AcqRel,
                },
                KeyPart::Sharing(Sharing::Exclusive),
            ])
            .unwrap(),
        );
        assert_eq!(
            select_class(&e, &cas).unwrap_err().variant(),
            "UnsupportedOrdering"
        );
        // A reader against a live writer is a concurrency shape.
        let reader = OperationClass::new(
            calibrated.id.clone(),
            ClassKey::new(vec![
                KeyPart::Atomic {
                    op: AtomicOp::Rmw,
                    ordering: Ordering::AcqRel,
                },
                KeyPart::Sharing(Sharing::WriterWithReaders),
            ])
            .unwrap(),
        );
        assert_eq!(
            select_class(&e, &reader).unwrap_err().variant(),
            "UnsupportedConcurrency"
        );
    }

    #[test]
    fn implementation_must_be_current() {
        let e = evidence();
        let c = select_class(&e, &transform(256)).unwrap();
        assert!(check_implementation(&c, &now_for(&[transform(256)])).is_ok());
        let mut n = now_for(&[transform(256)]);
        n.implementations[0].implementation.source_digest = "sha256:changed".into();
        assert_eq!(
            check_implementation(&c, &n).unwrap_err().variant(),
            "ImplementationStale"
        );
        let n = now_for(&[]);
        assert!(matches!(
            check_implementation(&c, &n),
            Err(Refusal::ImplementationStale { now: None, .. })
        ));
        // Runs that disagree on the implementation.
        let mut d = evidence();
        d.runs[1].calibrations[0].implementation.source_digest = "sha256:bb".into();
        assert_eq!(
            select_class(&d, &transform(256)).unwrap_err().variant(),
            "ProfilesDisagreeOnIdentity"
        );
    }

    #[test]
    fn placement_range_and_regime() {
        let c = select_class(&evidence(), &transform(256)).unwrap();
        let g = |v| Geometry::new(Parameter::ExtentBytes, v).unwrap();
        assert_eq!(
            place(&c, &g(16384), false),
            Ok(Placement::Measured { index: 0 })
        );
        assert_eq!(
            place(&c, &g(32768), false),
            Ok(Placement::Interpolated { below: 0 })
        );
        assert_eq!(
            place(&c, &g(131_072), false),
            Err(Refusal::CrossesRegimeBoundary {
                parameter: Parameter::ExtentBytes,
                value: 131_072,
                below: 65536,
                above: 262_144
            })
        );
        assert_eq!(
            place(&c, &g(131_072), true),
            Ok(Placement::Bracketed { below: 1 })
        );
        assert_eq!(
            place(&c, &g(524_288), false),
            Ok(Placement::Interpolated { below: 2 })
        );
        assert_eq!(
            place(&c, &g(8192), true),
            Err(Refusal::OutOfRange {
                parameter: Parameter::ExtentBytes,
                value: 8192,
                min: 16384,
                max: 1_048_576
            })
        );
        assert_eq!(
            place(&c, &g(1 << 21), true).unwrap_err().variant(),
            "OutOfRange"
        );
        assert_eq!(
            place(
                &c,
                &Geometry::new(Parameter::WorkingSetBytes, 32768).unwrap(),
                true
            ),
            Err(Refusal::MissingGeometry {
                expected: Parameter::ExtentBytes,
                given: Parameter::WorkingSetBytes
            })
        );
    }

    #[test]
    fn regime_needs_agreement_in_every_run() {
        let a = transform(256);
        let mut r0 = transform_curve(256);
        let mut r1 = transform_curve(256);
        // 16384 and 65536 agree in run 0; make them disagree in run 1 only.
        r0[1].median = 0.0087;
        r1[1].median = 0.0110;
        r1[1].spread = 0.001;
        r1[0].spread = 0.001;
        let e = Evidence {
            runs: vec![run(r0), run(r1)],
        };
        let c = select_class(&e, &a).unwrap();
        assert!(!one_regime(&c.points[0], &c.points[1]));
        let g = Geometry::new(Parameter::ExtentBytes, 32768).unwrap();
        assert_eq!(
            place(&c, &g, false).unwrap_err().variant(),
            "CrossesRegimeBoundary"
        );
    }

    #[test]
    fn single_point_curve_supports_only_its_point() {
        let a = transform(64);
        let e = Evidence {
            runs: vec![run(vec![cal(&a, Parameter::ExtentBytes, 4096, 0.13, 0.01)])],
        };
        let c = select_class(&e, &a).unwrap();
        let g = |v| Geometry::new(Parameter::ExtentBytes, v).unwrap();
        assert_eq!(
            place(&c, &g(4096), true),
            Ok(Placement::Measured { index: 0 })
        );
        assert_eq!(
            place(&c, &g(4097), true).unwrap_err().variant(),
            "OutOfRange"
        );
        assert_eq!(
            place(&c, &g(4095), true).unwrap_err().variant(),
            "OutOfRange"
        );
    }

    #[test]
    fn interpolation_and_bracketing() {
        let c = select_class(&evidence(), &transform(256)).unwrap();
        // Halfway in log2 between 16384 and 65536 is 32768.
        let i = interval(&c, &Placement::Interpolated { below: 0 }, 32768).unwrap();
        let mid = (c.points[0].central() + c.points[1].central()) / 2.0;
        assert!((i.central - mid).abs() < 1e-15);
        let a = Interval::of_point(&c.points[0].runs).unwrap();
        let b = Interval::of_point(&c.points[1].runs).unwrap();
        assert_eq!(i.lower, a.lower.min(b.lower));
        assert_eq!(i.upper, a.upper.max(b.upper));
        // At a measured point the interval is the point's.
        let m = interval(&c, &Placement::Measured { index: 2 }, 262_144).unwrap();
        assert_eq!(m, Interval::of_point(&c.points[2].runs).unwrap());
        // Bracketing: hull and geometric mean.
        let br = interval(&c, &Placement::Bracketed { below: 1 }, 131_072).unwrap();
        let g = (c.points[1].central() * c.points[2].central()).sqrt();
        assert!((br.central - g).abs() < 1e-15);
        assert!(br.lower <= br.central && br.central <= br.upper);
    }

    #[test]
    fn unstable_supporting_point_is_refused() {
        let mut e = evidence();
        e.runs[1].calibrations[1].status = CalibrationStatus::Unstable;
        let c = select_class(&e, &transform(256)).unwrap();
        assert_eq!(
            interval(&c, &Placement::Interpolated { below: 0 }, 32768),
            Err(Refusal::UnstableCalibration {
                calibration: "stream::BlockPass::transform.65536".into(),
                run: 1
            })
        );
        // The other side of the curve is unaffected.
        assert!(interval(&c, &Placement::Measured { index: 3 }, 1 << 20).is_ok());
    }
}
