//! Operation-class prediction (Unit 5; design: `docs/OPERATION_CLASS_PREDICTION.md`).
//!
//! Given an operation [`plan::Plan`] and one or more machine-profile runs converted to
//! [`evidence::Evidence`], select only calibration evidence that applies and return a time
//! interval with its evidence ([`predict::Prediction`]), or refuse with a machine-readable
//! [`refusal::Refusal`].
//!
//! ```text
//! identity  OperationId { package, path }, Implementation { version, source_digest }
//! class     ClassKey (sorted typed parts, one per kind) + OperationClass; exact equality only
//! geometry  one typed parameter per curve (ExtentBytes, WorkingSetBytes, …)
//! evidence  the plain profile evidence (runs, calibrations) and the caller's `Now`
//! selector  applicability → select_class → place → interval   (four separate steps)
//! interval  the §11 point interval and the Seq / Repeat / Par / Branch compositions
//! plan      leaves, counts (exact / expected / worst case / unknown), composition
//! predict   walks a plan and composes; never selects anything itself
//! refusal   every reason a prediction is refused, with data
//! ```
//!
//! The predictor does not know what measured its evidence: `o1-machine-profile` converts its
//! profiles into [`evidence::Evidence`]. Selection is exact equality of the whole class; there
//! is no nearest match and no extrapolation. Counts are integers with checked arithmetic; the
//! only conversion to `f64` is the final `count × ns_per_unit`.
//!
//! `no_std` + `alloc`, `#![forbid(unsafe_code)]`, depends on `o1-contract` only (and on
//! `serde`, derives only, behind the `serde` feature).

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
// A refusal carries the class it is about (by value, so it serialises as data); it is returned
// once per prediction, never in a loop, so its size is not boxed away.
#![allow(clippy::result_large_err)]

extern crate alloc;

pub mod class;
pub mod digest;
pub mod evidence;
pub mod geometry;
pub mod identity;
pub mod interval;
mod math;
pub mod plan;
pub mod predict;
pub mod refusal;
pub mod selector;

pub use class::{ClassKey, KeyPart, OperationClass, PartKind};
pub use evidence::{CalibrationEvidence, Evidence, Now, ProfileEvidence};
pub use geometry::{Geometry, Parameter};
pub use identity::{Implementation, OperationId};
pub use interval::Interval;
pub use plan::{Count, Leaf, Plan};
pub use predict::{Options, Prediction, predict, predict_paths};
pub use refusal::{PlanRefusal, Refusal};
