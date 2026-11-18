//! The vocabulary o1 uses to state how an operation executes and what it needs.
//!
//! ```text
//! Primitive + execution intent ──► ExecutionContract ──► ResourceEnvelope
//!                                     (later layers, not here: machine weights,
//!                                      predicted cost, verification, admission)
//! ```
//!
//! * [`Policy`] — zero-sized execution intent built by `const fn` setters
//!   (`Policy::new().streaming().non_temporal_write()`); contradictions do not compile.
//!   [`PolicyDesc`] is its plain-data mirror with the same validation.
//! * [`ExecutionContract`] — progress class, policy, control-state isolation, and a
//!   [`Cost`]; its [`Bound`] (O(1) / per-unit / fixed-capacity) is derived from the cost.
//! * [`ResourceEnvelope`] — one `u64` count per [`Quantity`]: totals add, the working-set
//!   peak combines by `max` or by sum depending on liveness. Checked arithmetic only.
//! * [`split_blocks`], [`lines_touched`], [`Cost::for_extent`] — size multipliers.
//!
//! No weights, no "compute units", no claim of cryptographic constant time or wall-clock
//! latency. `#![no_std]`, no `alloc`, no dependencies, `#![forbid(unsafe_code)]`.
//! Design: `docs/EXECUTION_CONTRACTS.md` in the repository.
//!
//! # Two jobs a single number cannot separate
//!
//! Job A streams 64 MiB through a 256-byte block transform with 4 KiB of hot state and no
//! synchronisation. Job B processes 2 MiB of 64-byte records, each with one atomic
//! read-modify-write on a 64 KiB table that other threads also update.
//!
//! ```
//! use core::num::NonZeroU64;
//! use o1_contract::{Cost, PeakGrowth, Quantity::*, ResourceEnvelope as E, Unit};
//!
//! let nz = |n| NonZeroU64::new(n).unwrap();
//! let job_a = Cost {
//!     fixed: E::ZERO.with(WorkingSet, 4096),
//!     per_unit: E::ZERO.with(Ops, 1).with(BytesRead, 256).with(BytesWritten, 256)
//!         .with(WorkingSet, 512),
//!     unit: Unit::Block { bytes: nz(256) },
//!     max_units: None,
//!     peak_growth: PeakGrowth::Fixed, // streaming: each block released before the next
//! }
//! .for_extent(0, 64 << 20)
//! .unwrap();
//!
//! let job_b = Cost {
//!     fixed: E::ZERO.with(WorkingSet, 64 << 10),
//!     per_unit: E::ZERO.with(Ops, 1).with(BytesRead, 64).with(AtomicRmws, 1)
//!         .with(SharedReads, 1).with(SharedWrites, 1).with(WorkingSet, 64),
//!     unit: Unit::Block { bytes: nz(64) },
//!     max_units: None,
//!     peak_growth: PeakGrowth::Fixed,
//! }
//! .for_extent(0, 2 << 20)
//! .unwrap();
//!
//! assert_eq!(job_a.to_string(),
//!     "ops=262144, bytes_read=67108864, bytes_written=67108864, working_set=4608");
//! assert_eq!(job_b.to_string(),
//!     "ops=32768, bytes_read=2097152, atomic_rmws=32768, shared_reads=32768, \
//!      shared_writes=32768, working_set=65600");
//! // Neither fits inside the other: they differ in kind, not only in size.
//! assert!(!job_a.within(&job_b) && !job_b.within(&job_a));
//! ```
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod contract;
pub mod cost;
pub mod envelope;
pub mod policy;

pub use contract::{Bound, ExecutionContract, Isolation, Progress};
pub use cost::{Blocks, Cost, PeakGrowth, Unit, lines_touched, split_blocks};
pub use envelope::{EnvelopeError, Exceeded, Kind, Quantity, ResourceEnvelope};
pub use policy::{
    Access, Axis, Policy, PolicyDesc, PolicyError, Reuse, Sharing, StaticPolicy, WritePolicy,
};
