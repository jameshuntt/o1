//! Refusal (design §12): an enum with data, never a string. A plan with one refused leaf is
//! refused whole; [`PlanRefusal`] names where.

use alloc::string::String;
use alloc::vec::Vec;

use crate::class::{OperationClass, PartKind};
use crate::geometry::Parameter;
use crate::identity::Implementation;

/// Why a profile is stale or unusable as evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum StaleReason {
    /// No profile run was supplied.
    NoProfile,
    /// The profile's schema is not the caller's.
    Schema {
        /// In the profile.
        profile: String,
        /// Now.
        now: String,
    },
    /// A build fact differs (toolchain, target, cargo profile, opt-level, flags).
    Build {
        /// Which fact.
        what: String,
        /// In the profile.
        profile: String,
        /// Now.
        now: String,
    },
    /// Taken from an unclean source scope.
    Unclean,
    /// The calibrating binary was not built from the source it read.
    NotBuiltFromSource,
    /// The evidence is malformed (its own validation failed).
    Malformed {
        /// What.
        what: String,
    },
}

/// Why a concurrent shape is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ConcurrencyReason {
    /// The class differs from every calibration in thread shape, isolation or sharing.
    Shape {
        /// The requested class.
        class: OperationClass,
        /// The parts that differ from the nearest calibration of the same operation.
        parts: Vec<PartKind>,
    },
    /// More `Par` branches than the profile's performance cores (or the count is unknown).
    TooManyBranches {
        /// Branches.
        branches: u64,
        /// Performance cores (smallest over the runs), when known.
        cores: Option<u32>,
    },
    /// A `Par` whose branches share writes.
    SharedWrites,
}

/// Work the predictor never prices (design §19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum UnpricedKind {
    /// A system call / kernel entry.
    KernelEntry,
    /// A heap allocation.
    Allocation,
    /// An external process.
    ExternalProcess,
    /// GPU work.
    Gpu,
    /// Cryptography.
    Cryptography,
    /// Waiting on another thread (mutex, condvar, spin wait).
    Waiting,
    /// Blocking I/O.
    Blocking,
    /// Non-temporal writes (no backend exists).
    NonTemporalWrite,
}

/// Why a prediction is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Refusal {
    /// No calibration has this operation id.
    NoCalibration {
        /// The requested class.
        class: OperationClass,
    },
    /// Same operation id, a key part differs (none of the specific variants below applies).
    ClassMismatch {
        /// The requested class.
        class: OperationClass,
        /// The parts that differ from the nearest calibration.
        parts: Vec<PartKind>,
    },
    /// The differing part is the atomic operation or ordering.
    UnsupportedOrdering {
        /// The requested class.
        class: OperationClass,
        /// The parts that differ.
        parts: Vec<PartKind>,
    },
    /// The differing part is the access pattern or visiting order.
    UnsupportedAccessPattern {
        /// The requested class.
        class: OperationClass,
        /// The parts that differ.
        parts: Vec<PartKind>,
    },
    /// Threads / isolation / sharing differ, or a `Par` rule.
    UnsupportedConcurrency {
        /// Why.
        reason: ConcurrencyReason,
    },
    /// Geometry outside the curve's [min, max].
    OutOfRange {
        /// Parameter.
        parameter: Parameter,
        /// Requested value.
        value: u64,
        /// Smallest measured point.
        min: u64,
        /// Largest measured point.
        max: u64,
    },
    /// Between two adjacent points that are not one regime.
    CrossesRegimeBoundary {
        /// Parameter.
        parameter: Parameter,
        /// Requested value.
        value: u64,
        /// The point below.
        below: u64,
        /// The point above.
        above: u64,
    },
    /// The leaf's parameter is not the curve's.
    MissingGeometry {
        /// The curve's parameter.
        expected: Parameter,
        /// The leaf's.
        given: Parameter,
    },
    /// A supporting point is not stable in a supplied run.
    UnstableCalibration {
        /// Calibration id.
        calibration: String,
        /// Run index.
        run: usize,
    },
    /// The profile is of another machine (class).
    MachineMismatch {
        /// Profile identity digest.
        profile: String,
        /// Now.
        now: String,
    },
    /// Schema, build or provenance.
    StaleProfile {
        /// Why.
        reason: StaleReason,
    },
    /// The calibration method changed.
    MethodStale {
        /// Profile method digest.
        profile: String,
        /// Now.
        now: String,
    },
    /// The measured code changed (names the class).
    ImplementationStale {
        /// The class.
        class: OperationClass,
        /// What was measured.
        measured: Implementation,
        /// What is there now (none: the caller did not say).
        now: Option<Implementation>,
    },
    /// The supplied runs are not one machine, one method or one implementation.
    ProfilesDisagreeOnIdentity {
        /// What differs.
        what: String,
    },
    /// A loop or retry without a bound (`Count::Unknown(Unbounded)`).
    UnboundedWork,
    /// A count or path depends on data the plan does not have.
    DataDependentUnknown {
        /// What.
        what: String,
    },
    /// The cost depends on earlier calls.
    HistoryDependent {
        /// What.
        what: String,
    },
    /// Conditional paths without evidence-backed probabilities.
    UnknownBranch,
    /// Work the predictor never prices.
    UnpricedComponent {
        /// Its kind.
        kind: UnpricedKind,
    },
    /// Caller-supplied code with no calibration.
    CallbackUnpriced {
        /// The callback's identity.
        callback: String,
    },
    /// Checked arithmetic failed, or the final product is not finite.
    Overflow,
    /// Nothing to predict.
    EmptyPlan,
}

impl Refusal {
    /// The variant's name, as the design lists it.
    pub fn variant(&self) -> &'static str {
        match self {
            Self::NoCalibration { .. } => "NoCalibration",
            Self::ClassMismatch { .. } => "ClassMismatch",
            Self::UnsupportedOrdering { .. } => "UnsupportedOrdering",
            Self::UnsupportedAccessPattern { .. } => "UnsupportedAccessPattern",
            Self::UnsupportedConcurrency { .. } => "UnsupportedConcurrency",
            Self::OutOfRange { .. } => "OutOfRange",
            Self::CrossesRegimeBoundary { .. } => "CrossesRegimeBoundary",
            Self::MissingGeometry { .. } => "MissingGeometry",
            Self::UnstableCalibration { .. } => "UnstableCalibration",
            Self::MachineMismatch { .. } => "MachineMismatch",
            Self::StaleProfile { .. } => "StaleProfile",
            Self::MethodStale { .. } => "MethodStale",
            Self::ImplementationStale { .. } => "ImplementationStale",
            Self::ProfilesDisagreeOnIdentity { .. } => "ProfilesDisagreeOnIdentity",
            Self::UnboundedWork => "UnboundedWork",
            Self::DataDependentUnknown { .. } => "DataDependentUnknown",
            Self::HistoryDependent { .. } => "HistoryDependent",
            Self::UnknownBranch => "UnknownBranch",
            Self::UnpricedComponent { .. } => "UnpricedComponent",
            Self::CallbackUnpriced { .. } => "CallbackUnpriced",
            Self::Overflow => "Overflow",
            Self::EmptyPlan => "EmptyPlan",
        }
    }

    /// Every variant name, in the design's order.
    pub const VARIANTS: [&'static str; 22] = [
        "NoCalibration",
        "ClassMismatch",
        "UnsupportedOrdering",
        "UnsupportedAccessPattern",
        "UnsupportedConcurrency",
        "OutOfRange",
        "CrossesRegimeBoundary",
        "MissingGeometry",
        "UnstableCalibration",
        "MachineMismatch",
        "StaleProfile",
        "MethodStale",
        "ImplementationStale",
        "ProfilesDisagreeOnIdentity",
        "UnboundedWork",
        "DataDependentUnknown",
        "HistoryDependent",
        "UnknownBranch",
        "UnpricedComponent",
        "CallbackUnpriced",
        "Overflow",
        "EmptyPlan",
    ];

    /// The refusal for a class that differs from the nearest calibration of its operation in
    /// `parts` (precedence: atomic op / ordering; access / order; threads / isolation /
    /// sharing; callback; anything else).
    pub fn for_differing(class: &OperationClass, parts: Vec<PartKind>) -> Self {
        let has = |k: &[PartKind]| parts.iter().any(|p| k.contains(p));
        if has(&[PartKind::Atomic]) {
            Self::UnsupportedOrdering {
                class: class.clone(),
                parts,
            }
        } else if has(&[PartKind::Access, PartKind::Order]) {
            Self::UnsupportedAccessPattern {
                class: class.clone(),
                parts,
            }
        } else if has(&[PartKind::Threads, PartKind::Isolation, PartKind::Sharing]) {
            Self::UnsupportedConcurrency {
                reason: ConcurrencyReason::Shape {
                    class: class.clone(),
                    parts,
                },
            }
        } else if has(&[PartKind::Callback]) {
            let callback = match class.key.get(PartKind::Callback) {
                Some(crate::class::KeyPart::Callback(c)) => c.clone(),
                _ => String::new(),
            };
            Self::CallbackUnpriced { callback }
        } else {
            Self::ClassMismatch {
                class: class.clone(),
                parts,
            }
        }
    }
}

/// A refused plan: the refusal and the leaf (or plan node) it arose at.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlanRefusal {
    /// The leaf's name, or the node's path (`/1/repeat`), or none for the profile as a whole.
    pub leaf: Option<String>,
    /// Why.
    pub refusal: Refusal,
}

impl PlanRefusal {
    /// A refusal of the whole evidence (no leaf).
    pub fn whole(refusal: Refusal) -> Self {
        Self {
            leaf: None,
            refusal,
        }
    }

    /// A refusal at `leaf`.
    pub fn at(leaf: impl Into<String>, refusal: Refusal) -> Self {
        Self {
            leaf: Some(leaf.into()),
            refusal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::class::{ClassKey, KeyPart};
    use crate::identity::OperationId;
    use alloc::vec;

    fn class(parts: Vec<KeyPart>) -> OperationClass {
        OperationClass::new(OperationId::new("p", "f"), ClassKey::new(parts).unwrap())
    }

    #[test]
    fn variant_names_are_the_designs() {
        assert_eq!(Refusal::UnboundedWork.variant(), "UnboundedWork");
        assert_eq!(Refusal::VARIANTS.len(), 22);
        for (i, a) in Refusal::VARIANTS.iter().enumerate() {
            for b in &Refusal::VARIANTS[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn differing_part_precedence() {
        let c = class(vec![KeyPart::Callback("user".into())]);
        let r = Refusal::for_differing(&c, vec![PartKind::Atomic, PartKind::Sharing]);
        assert_eq!(r.variant(), "UnsupportedOrdering");
        let r = Refusal::for_differing(&c, vec![PartKind::Sharing, PartKind::Order]);
        assert_eq!(r.variant(), "UnsupportedAccessPattern");
        let r = Refusal::for_differing(&c, vec![PartKind::Threads]);
        assert_eq!(r.variant(), "UnsupportedConcurrency");
        let r = Refusal::for_differing(&c, vec![PartKind::Callback, PartKind::BlockBytes]);
        assert_eq!(
            r,
            Refusal::CallbackUnpriced {
                callback: "user".into()
            }
        );
        let r = Refusal::for_differing(&c, vec![PartKind::BlockBytes]);
        assert_eq!(r.variant(), "ClassMismatch");
    }
}
