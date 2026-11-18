//! Execution intent: the zero-sized [`Policy`] and its plain-data mirror [`PolicyDesc`].
//!
//! Four axes, each holding at most one value:
//!
//! | axis | values | static setter |
//! |---|---|---|
//! | access | [`Access::Sequential`], [`Access::Random`] | [`sequential`](Policy::sequential), [`random`](Policy::random) |
//! | reuse | [`Reuse::Once`], [`Reuse::Reused`] | [`reuse_once`](Policy::reuse_once), [`reused`](Policy::reused) |
//! | sharing | [`Sharing::Exclusive`], [`Sharing::SharedRead`], [`Sharing::Handoff`] | none — see below |
//! | write | [`WritePolicy::Ordinary`], [`WritePolicy::NonTemporal`] | [`non_temporal_write`](Policy::non_temporal_write) |
//!
//! [`streaming`](Policy::streaming) sets `Sequential` + `Once`.
//!
//! A modifier is a **semantic request**: "this is the intended access, reuse or write
//! policy; a backend may choose any implementation that gives the same result". It is
//! never a promise about hardware. A non-temporal write in particular is a hint: it does
//! not guarantee that data bypasses every cache level.
//!
//! **Sharing has no static setter.** In Rust the borrow in an operation's signature already
//! states ownership and the compiler proves it (`&mut` = exclusive, `&` = shared read, an
//! SPSC `split(&mut self)` = handoff); a static modifier could only repeat or contradict a
//! proven fact. Sharing appears in [`PolicyDesc`], where a primitive reports it as a
//! structural fact and a planner uses it to describe a job.
//!
//! # Static form
//!
//! ```
//! use o1_contract::{Access, Policy, Reuse, StaticPolicy, WritePolicy};
//! let p = Policy::new().streaming().non_temporal_write();
//! assert_eq!(core::mem::size_of_val(&p), 0);
//! let d = p.desc();
//! assert_eq!(d.access, Some(Access::Sequential));
//! assert_eq!(d.reuse, Some(Reuse::Once));
//! assert_eq!(d.write, Some(WritePolicy::NonTemporal));
//! ```
//!
//! Setting an axis twice does not compile (the setter only exists while the axis is unset):
//!
//! ```compile_fail,E0599
//! let _ = o1_contract::Policy::new().sequential().random();
//! ```
//! ```compile_fail,E0599
//! let _ = o1_contract::Policy::new().reuse_once().reused();
//! ```
//! ```compile_fail,E0599
//! let _ = o1_contract::Policy::new().sequential().streaming(); // access already set
//! ```
//!
//! A real contradiction does not compile, in either order (the setter evaluates the
//! resulting type's [`StaticPolicy::DESC`], which runs [`PolicyDesc::validate`]):
//!
//! ```compile_fail,E0080
//! let _ = o1_contract::Policy::new().reused().non_temporal_write();
//! ```
//! ```compile_fail,E0080
//! let _ = o1_contract::Policy::new().non_temporal_write().reused();
//! ```
//!
//! A marker in the wrong position is a type error:
//!
//! ```compile_fail,E0277
//! use o1_contract::{Policy, StaticPolicy, policy::Reused};
//! let _ = <Policy<Reused> as StaticPolicy>::DESC; // `Reused` is not an access marker
//! ```

use core::fmt;
use core::marker::PhantomData;

/// Access pattern over the data an operation touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Access {
    /// Addresses in increasing order, each unit after the previous one.
    Sequential,
    /// Addresses chosen by data (a handle, an index, a hash).
    Random,
}

/// Whether the bytes an operation touches (read or written) are expected to be touched
/// again soon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reuse {
    /// Touched once and not again (streaming).
    Once,
    /// Expected to be touched again.
    Reused,
}

/// How the data an operation touches is shared with other threads while it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sharing {
    /// No other thread accesses it.
    Exclusive,
    /// Other threads may read it; nobody writes it during the operation.
    SharedRead,
    /// Two sides; every location is written by one side and read by the other
    /// (producer-owned and consumer-owned state).
    Handoff,
}

/// How written bytes are intended to be stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WritePolicy {
    /// Ordinary stores. What every o1 implementation uses today.
    Ordinary,
    /// The written bytes will not be read again soon; a backend may use non-temporal
    /// stores. A hint, never a guarantee that data bypasses the caches.
    NonTemporal,
}

/// One of the four axes, named in [`PolicyError::ContradictsStructure`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// [`Access`]
    Access,
    /// [`Reuse`]
    Reuse,
    /// [`Sharing`]
    Sharing,
    /// [`WritePolicy`]
    Write,
}

/// Why a policy is contradictory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolicyError {
    /// `NonTemporal` writes with `Reused` data: a non-temporal write *is* the statement that
    /// the written bytes will not be reused soon.
    NonTemporalReused,
    /// A write policy on `SharedRead` data, which nobody writes during the operation.
    WriteOnSharedRead,
    /// The request differs from what the operation structurally does on this axis.
    ContradictsStructure {
        /// The axis on which they differ.
        axis: Axis,
    },
}

impl PolicyError {
    /// Stops const evaluation with this error's message (post-monomorphisation `E0080`).
    const fn fail(self) -> ! {
        match self {
            Self::NonTemporalReused => {
                panic!("policy contradiction: non-temporal writes on reused data")
            }
            Self::WriteOnSharedRead => {
                panic!("policy contradiction: a write policy on shared-read (unwritten) data")
            }
            Self::ContradictsStructure { .. } => {
                panic!("policy contradiction: the request differs from the operation's structure")
            }
        }
    }
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonTemporalReused => f.write_str("non-temporal writes on reused data"),
            Self::WriteOnSharedRead => f.write_str("a write policy on shared-read data"),
            Self::ContradictsStructure { axis } => {
                write!(
                    f,
                    "request contradicts the operation's structure ({axis:?})"
                )
            }
        }
    }
}

impl core::error::Error for PolicyError {}

/// Plain-data description of execution intent and ownership. `None` = no statement.
///
/// In a primitive's published contract every `Some` sharing and access value is a
/// structural fact of that primitive's code; reuse and write values are requests (from a
/// caller's [`Policy`]). This is the form a planner reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PolicyDesc {
    /// Access pattern.
    pub access: Option<Access>,
    /// Reuse expectation.
    pub reuse: Option<Reuse>,
    /// Sharing / ownership.
    pub sharing: Option<Sharing>,
    /// Write policy.
    pub write: Option<WritePolicy>,
}

/// Merges one axis of a request onto a structure; differing values are an error.
macro_rules! merge_axis {
    ($req:expr, $st:expr, $axis:expr) => {
        match ($req, $st) {
            (Some(r), Some(s)) => {
                if r as u8 == s as u8 {
                    Some(s)
                } else {
                    return Err(PolicyError::ContradictsStructure { axis: $axis });
                }
            }
            (Some(r), None) => Some(r),
            (None, s) => s,
        }
    };
}

impl PolicyDesc {
    /// No statement on any axis.
    pub const NONE: Self = Self {
        access: None,
        reuse: None,
        sharing: None,
        write: None,
    };

    /// Checks the two cross-axis contradictions. This one function is used by the static
    /// form (at compile time) and by dynamic callers, so the two cannot disagree.
    ///
    /// # Errors
    ///
    /// [`PolicyError::NonTemporalReused`], [`PolicyError::WriteOnSharedRead`].
    pub const fn validate(&self) -> Result<(), PolicyError> {
        if matches!(self.write, Some(WritePolicy::NonTemporal))
            && matches!(self.reuse, Some(Reuse::Reused))
        {
            return Err(PolicyError::NonTemporalReused);
        }
        if self.write.is_some() && matches!(self.sharing, Some(Sharing::SharedRead)) {
            return Err(PolicyError::WriteOnSharedRead);
        }
        Ok(())
    }

    /// Merges this request onto an operation's `structure` and validates the result.
    /// Returns the merged description (structure plus the request's extra statements).
    ///
    /// # Errors
    ///
    /// [`PolicyError::ContradictsStructure`] if an axis is set in both and differs, else
    /// whatever [`validate`](Self::validate) reports for the merged description.
    pub const fn validate_against(&self, structure: &Self) -> Result<Self, PolicyError> {
        let merged = Self {
            access: merge_axis!(self.access, structure.access, Axis::Access),
            reuse: merge_axis!(self.reuse, structure.reuse, Axis::Reuse),
            sharing: merge_axis!(self.sharing, structure.sharing, Axis::Sharing),
            write: merge_axis!(self.write, structure.write, Axis::Write),
        };
        match merged.validate() {
            Ok(()) => Ok(merged),
            Err(e) => Err(e),
        }
    }

    /// [`validate_against`](Self::validate_against) for constant contexts: a contradiction
    /// stops compilation (`E0080`).
    ///
    /// # Panics
    ///
    /// On any [`PolicyError`]; evaluated in a `const` item that is a compile-time error.
    pub const fn merged_or_fail(&self, structure: &Self) -> Self {
        match self.validate_against(structure) {
            Ok(m) => m,
            Err(e) => e.fail(),
        }
    }
}

mod sealed {
    pub trait Sealed {}
}

/// A marker that may stand in the position of axis `A` in a [`Policy`]. Sealed.
pub trait Mark<A>: sealed::Sealed {
    /// The value this marker states on axis `A` (`None` for [`Unset`]).
    const VALUE: Option<A>;
}

/// Marker: axis not stated.
#[derive(Debug)]
pub enum Unset {}
/// Marker: [`Access::Sequential`].
#[derive(Debug)]
pub enum Sequential {}
/// Marker: [`Access::Random`].
#[derive(Debug)]
pub enum Random {}
/// Marker: [`Reuse::Once`].
#[derive(Debug)]
pub enum ReuseOnce {}
/// Marker: [`Reuse::Reused`].
#[derive(Debug)]
pub enum Reused {}
/// Marker: [`WritePolicy::NonTemporal`].
#[derive(Debug)]
pub enum NonTemporalWrite {}

impl sealed::Sealed for Unset {}
impl sealed::Sealed for Sequential {}
impl sealed::Sealed for Random {}
impl sealed::Sealed for ReuseOnce {}
impl sealed::Sealed for Reused {}
impl sealed::Sealed for NonTemporalWrite {}

impl<A> Mark<A> for Unset {
    const VALUE: Option<A> = None;
}
impl Mark<Access> for Sequential {
    const VALUE: Option<Access> = Some(Access::Sequential);
}
impl Mark<Access> for Random {
    const VALUE: Option<Access> = Some(Access::Random);
}
impl Mark<Reuse> for ReuseOnce {
    const VALUE: Option<Reuse> = Some(Reuse::Once);
}
impl Mark<Reuse> for Reused {
    const VALUE: Option<Reuse> = Some(Reuse::Reused);
}
impl Mark<WritePolicy> for NonTemporalWrite {
    const VALUE: Option<WritePolicy> = Some(WritePolicy::NonTemporal);
}

/// A zero-sized execution policy: access `A`, reuse `R`, write `W`. See the
/// [module docs](self).
pub struct Policy<A = Unset, R = Unset, W = Unset>(PhantomData<Markers<A, R, W>>);

/// `fn() -> _` keeps `Policy` `Send`, `Sync` and covariant whatever the markers are.
type Markers<A, R, W> = fn() -> (A, R, W);

/// A [`Policy`] type, with its description as a constant. Sealed.
pub trait StaticPolicy: sealed::Sealed + Copy {
    /// The policy's description, validated at compile time.
    const DESC: PolicyDesc;

    /// [`DESC`](Self::DESC), from a value.
    #[inline]
    fn desc(self) -> PolicyDesc {
        Self::DESC
    }
}

impl<A, R, W> sealed::Sealed for Policy<A, R, W> {}

impl<A: Mark<Access>, R: Mark<Reuse>, W: Mark<WritePolicy>> StaticPolicy for Policy<A, R, W> {
    const DESC: PolicyDesc = PolicyDesc {
        access: A::VALUE,
        reuse: R::VALUE,
        sharing: None,
        write: W::VALUE,
    }
    .merged_or_fail(&PolicyDesc::NONE);
}

impl<A: Mark<Access>, R: Mark<Reuse>, W: Mark<WritePolicy>> Policy<A, R, W> {
    /// Builds the policy value after forcing its validation.
    #[inline]
    const fn checked() -> Self {
        let _validated: PolicyDesc = <Self as StaticPolicy>::DESC;
        Policy(PhantomData)
    }
}

impl Policy {
    /// The policy that states nothing.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Policy(PhantomData)
    }
}

impl<R: Mark<Reuse>, W: Mark<WritePolicy>> Policy<Unset, R, W> {
    /// Access in increasing address order.
    #[inline]
    #[must_use]
    pub const fn sequential(self) -> Policy<Sequential, R, W> {
        Policy::checked()
    }

    /// Data-chosen addresses.
    #[inline]
    #[must_use]
    pub const fn random(self) -> Policy<Random, R, W> {
        Policy::checked()
    }
}

impl<A: Mark<Access>, W: Mark<WritePolicy>> Policy<A, Unset, W> {
    /// The touched bytes are used once.
    #[inline]
    #[must_use]
    pub const fn reuse_once(self) -> Policy<A, ReuseOnce, W> {
        Policy::checked()
    }

    /// The touched bytes are expected to be used again.
    #[inline]
    #[must_use]
    pub const fn reused(self) -> Policy<A, Reused, W> {
        Policy::checked()
    }
}

impl<W: Mark<WritePolicy>> Policy<Unset, Unset, W> {
    /// Sequential access, each byte used once: [`sequential`](Policy::sequential) +
    /// [`reuse_once`](Policy::reuse_once).
    #[inline]
    #[must_use]
    pub const fn streaming(self) -> Policy<Sequential, ReuseOnce, W> {
        Policy::checked()
    }
}

impl<A: Mark<Access>, R: Mark<Reuse>> Policy<A, R, Unset> {
    /// The written bytes will not be read again soon; a backend may use non-temporal stores.
    #[inline]
    #[must_use]
    pub const fn non_temporal_write(self) -> Policy<A, R, NonTemporalWrite> {
        Policy::checked()
    }
}

impl<A, R, W> Clone for Policy<A, R, W> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<A, R, W> Copy for Policy<A, R, W> {}

impl Default for Policy {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Mark<Access>, R: Mark<Reuse>, W: Mark<WritePolicy>> fmt::Debug for Policy<A, R, W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Policy").field(&Self::DESC).finish()
    }
}
