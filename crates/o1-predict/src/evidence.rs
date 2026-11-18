//! The plain evidence type (design §13): one or more profile runs, each with its identity,
//! method and provenance facts and its calibrations; and [`Now`], the facts the caller supplies
//! about the machine and the code at prediction time. Nothing here measures or reads files:
//! `o1-machine-profile` converts its profiles into these values.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::class::OperationClass;
use crate::geometry::Parameter;
use crate::identity::{Implementation, OperationId};

/// A calibration's status in its run (Unit 4's verdicts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum CalibrationStatus {
    /// Stable by Unit 4's rule.
    Stable,
    /// Half or more rounds disturbed, or spread ≥ 1.
    Unstable,
    /// Time did not follow work.
    NotScaling,
    /// A sample below the timer floor.
    BelowTimerFloor,
    /// A result check failed.
    CheckFailed,
}

/// One calibration point of one run.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CalibrationEvidence {
    /// Calibration id in its profile.
    pub id: String,
    /// The class it measured.
    pub class: OperationClass,
    /// The code it measured.
    pub implementation: Implementation,
    /// The curve parameter.
    pub parameter: Parameter,
    /// Its value at this point.
    pub value: u64,
    /// Unit of work (`byte`, `op`, …); a time is ns per one of these.
    pub unit: String,
    /// Pooled median, ns per unit.
    pub median: f64,
    /// Relative spread (Unit 4's rule).
    pub spread: f64,
    /// Smallest per-round median, ns per unit.
    pub round_low: f64,
    /// Largest per-round median, ns per unit.
    pub round_high: f64,
    /// Status.
    pub status: CalibrationStatus,
    /// Conditions summary (load, power) of the rounds that measured it.
    pub conditions: String,
}

/// Build facts of the calibrating binary.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BuildFacts {
    /// `rustc -V`.
    pub toolchain: String,
    /// Target triple.
    pub target: String,
    /// Cargo profile.
    pub cargo_profile: String,
    /// Optimisation level.
    pub opt_level: String,
    /// `RUSTFLAGS` as cargo encoded them.
    pub rustflags: String,
}

/// One profile run.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileEvidence {
    /// Profile schema.
    pub schema: String,
    /// Machine identity digest.
    pub identity_digest: String,
    /// Environment (OS, kernel) digest; reported, never a refusal (design §14).
    pub environment_digest: String,
    /// Calibration method digest.
    pub method_digest: String,
    /// Git revision the run was taken at.
    pub revision: String,
    /// Taken from a clean source scope.
    pub clean: bool,
    /// The binary was built from the source it read.
    pub built_from_source: bool,
    /// Build facts.
    pub build: BuildFacts,
    /// Performance cores of the machine (the `Par` limit, §9b), when known.
    pub performance_cores: Option<u32>,
    /// Conditions summary of the run.
    pub conditions: String,
    /// The calibrations.
    pub calibrations: Vec<CalibrationEvidence>,
}

/// The evidence a prediction rests on: one or more runs.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Evidence {
    /// The runs.
    pub runs: Vec<ProfileEvidence>,
}

/// The implementation of one operation now.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CurrentImplementation {
    /// The operation.
    pub id: OperationId,
    /// Its code now.
    pub implementation: Implementation,
}

/// What the caller says about the machine and the code now.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Now {
    /// The profile schema the caller reads.
    pub schema: String,
    /// This machine's identity digest.
    pub identity_digest: String,
    /// This machine's environment digest (reported only).
    pub environment_digest: String,
    /// The calibration method digest now.
    pub method_digest: String,
    /// Build facts of the binary that would calibrate now.
    pub build: BuildFacts,
    /// Every operation's implementation now.
    pub implementations: Vec<CurrentImplementation>,
}

impl Now {
    /// The implementation of `id` now.
    pub fn implementation(&self, id: &OperationId) -> Option<&Implementation> {
        self.implementations
            .iter()
            .find(|c| &c.id == id)
            .map(|c| &c.implementation)
    }
}

/// Why evidence is malformed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    /// Two calibrations of one run have the same class and parameter value.
    DuplicatePoint {
        /// Run index.
        run: usize,
        /// The second calibration's id.
        id: String,
    },
    /// A calibration's value is zero, or its numbers are not finite and positive (median,
    /// round range) or not finite and non-negative (spread), or its round range is inverted.
    BadNumber {
        /// Run index.
        run: usize,
        /// Calibration id.
        id: String,
    },
    /// One class with two parameters or two units in one run.
    MixedCurve {
        /// Run index.
        run: usize,
        /// Calibration id.
        id: String,
    },
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePoint { run, id } => {
                write!(f, "run {run}: {id} repeats a class and value")
            }
            Self::BadNumber { run, id } => write!(f, "run {run}: {id} has an invalid number"),
            Self::MixedCurve { run, id } => {
                write!(f, "run {run}: {id} mixes parameters or units in one class")
            }
        }
    }
}

impl Evidence {
    /// Checks every run: positive finite numbers, one point per class and value, one parameter
    /// and unit per class.
    ///
    /// # Errors
    ///
    /// [`EvidenceError`].
    pub fn validate(&self) -> Result<(), EvidenceError> {
        for (r, run) in self.runs.iter().enumerate() {
            for (i, c) in run.calibrations.iter().enumerate() {
                let finite_pos = |x: f64| x.is_finite() && x > 0.0;
                if c.value == 0
                    || !finite_pos(c.median)
                    || !finite_pos(c.round_low)
                    || !finite_pos(c.round_high)
                    || c.round_low > c.round_high
                    || !c.spread.is_finite()
                    || c.spread < 0.0
                {
                    return Err(EvidenceError::BadNumber {
                        run: r,
                        id: c.id.clone(),
                    });
                }
                for o in &run.calibrations[..i] {
                    if o.class == c.class {
                        if o.value == c.value {
                            return Err(EvidenceError::DuplicatePoint {
                                run: r,
                                id: c.id.clone(),
                            });
                        }
                        if o.parameter != c.parameter || o.unit != c.unit {
                            return Err(EvidenceError::MixedCurve {
                                run: r,
                                id: c.id.clone(),
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
