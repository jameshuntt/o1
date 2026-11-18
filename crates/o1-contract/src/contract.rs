//! [`ExecutionContract`]: the structural execution characteristics of one operation.

use crate::cost::{Cost, Unit};
use crate::envelope::{EnvelopeError, ResourceEnvelope};
use crate::policy::{PolicyDesc, PolicyError};

/// How an operation makes progress relative to other threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Progress {
    /// Performs no synchronisation; Rust borrows rule out concurrent mutation (`&mut self`
    /// operations, or `&self` reads of data nobody writes meanwhile).
    NoSynchronization,
    /// Finishes in a bounded number of its own steps whatever other threads do (no loop
    /// waiting on another thread, no compare-exchange retry).
    WaitFree,
    /// Some thread always makes progress; this one may retry.
    LockFree,
    /// May wait for another thread or the kernel.
    Blocking,
}

/// Whether the control state an operation writes for another thread sits on its own
/// padded unit (false-sharing avoidance as a layout fact).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Isolation {
    /// No claim.
    NoClaim,
    /// Each side's written control words sit alone in a unit of `unit_bytes` bytes
    /// (alignment and size of `CachePadded` on the target the contract was compiled for).
    Padded {
        /// Bytes per padded unit.
        unit_bytes: u64,
    },
}

/// The algorithmic bound, derived from a [`Cost`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bound {
    /// O(1): one execution's envelope depends on no size.
    Constant,
    /// Linear in a size the caller supplies.
    PerUnit(Unit),
    /// Linear in a size that cannot exceed `max` (a fixed capacity or buffer length).
    Capacity {
        /// The unit.
        unit: Unit,
        /// The fixed maximum.
        max: u64,
    },
}

/// The structural execution characteristics of one operation.
///
/// Not claimable, so absent: cryptographic constant time, bounded wall-clock latency.
/// Unbounded operations (waiting loops, kernel waits) have no contract: a [`Cost`] is
/// finite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutionContract {
    /// Progress class.
    pub progress: Progress,
    /// Access, reuse, sharing and write statements (structural facts and requests).
    pub policy: PolicyDesc,
    /// Control-state isolation.
    pub isolation: Isolation,
    /// The cost formula of one execution.
    pub cost: Cost,
}

impl ExecutionContract {
    /// The algorithmic bound, derived from [`cost`](Self::cost).
    #[must_use]
    pub const fn bound(&self) -> Bound {
        if self.cost.per_unit.is_zero() {
            Bound::Constant
        } else {
            match self.cost.max_units {
                Some(max) => Bound::Capacity {
                    unit: self.cost.unit,
                    max,
                },
                None => Bound::PerUnit(self.cost.unit),
            }
        }
    }

    /// Validates the policy (see [`PolicyDesc::validate`]).
    ///
    /// # Errors
    ///
    /// The policy's [`PolicyError`].
    pub const fn validate(&self) -> Result<(), PolicyError> {
        self.policy.validate()
    }

    /// `self` if valid; for publishing contracts as constants.
    ///
    /// # Panics
    ///
    /// If [`validate`](Self::validate) fails — in a `const` item, a compile-time error.
    #[must_use]
    pub const fn assert_valid(self) -> Self {
        if self.validate().is_err() {
            panic!("invalid execution contract: contradictory policy");
        }
        self
    }

    /// The envelope of one execution over `units` units ([`Cost::at`]).
    ///
    /// # Errors
    ///
    /// As [`Cost::at`].
    pub const fn envelope(&self, units: u64) -> Result<ResourceEnvelope, EnvelopeError> {
        self.cost.at(units)
    }
}
