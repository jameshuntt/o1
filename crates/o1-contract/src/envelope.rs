//! [`ResourceEnvelope`]: countable resource quantities and their combination rules.
//!
//! No quantity has a weight. Two envelopes are compared quantity by quantity
//! ([`within`](ResourceEnvelope::within)); they are never reduced to one number.
//!
//! All arithmetic is `const fn` and **checked**: a count that does not fit `u64` is
//! [`EnvelopeError::Overflow`] naming the quantity. Nothing wraps; nothing saturates.

use core::fmt;

/// How a quantity combines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A count that adds whenever operations add.
    Total,
    /// A peak: combines by `max` when footprints are not live together
    /// ([`then`](ResourceEnvelope::then), [`repeat`](ResourceEnvelope::repeat)), by sum
    /// when they are ([`alongside`](ResourceEnvelope::alongside),
    /// [`repeat_retained`](ResourceEnvelope::repeat_retained)).
    Peak,
}

/// One countable resource quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Quantity {
    /// Executions of bounded primitive steps.
    Ops,
    /// Bytes of caller data read (payload moved out of a primitive, or data a pass hands
    /// to its callback to read). A primitive's own control fields are not counted.
    BytesRead,
    /// Bytes of caller data written (payload moved into a primitive, or output a pass
    /// hands to its callback to write).
    BytesWritten,
    /// Atomic load instructions.
    AtomicLoads,
    /// Atomic store instructions.
    AtomicStores,
    /// Atomic read-modify-write instructions (swap, compare-exchange, fetch-op).
    AtomicRmws,
    /// Reads (plain or atomic) of memory another thread writes by design.
    SharedReads,
    /// Writes (plain or atomic) to memory another thread reads by design: the
    /// coherence-sensitive writes.
    SharedWrites,
    /// System calls / kernel entries.
    KernelEntries,
    /// Heap allocations.
    Allocations,
    /// Peak bytes the operation may address that must stay resident for its stated reuse.
    WorkingSet,
}

impl Quantity {
    /// Number of quantities.
    pub const COUNT: usize = 11;

    /// Every quantity, in declaration order (index = discriminant).
    pub const ALL: [Quantity; Self::COUNT] = [
        Self::Ops,
        Self::BytesRead,
        Self::BytesWritten,
        Self::AtomicLoads,
        Self::AtomicStores,
        Self::AtomicRmws,
        Self::SharedReads,
        Self::SharedWrites,
        Self::KernelEntries,
        Self::Allocations,
        Self::WorkingSet,
    ];

    /// The combination rule.
    #[inline]
    #[must_use]
    pub const fn kind(self) -> Kind {
        match self {
            Self::WorkingSet => Kind::Peak,
            _ => Kind::Total,
        }
    }

    /// `snake_case` name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ops => "ops",
            Self::BytesRead => "bytes_read",
            Self::BytesWritten => "bytes_written",
            Self::AtomicLoads => "atomic_loads",
            Self::AtomicStores => "atomic_stores",
            Self::AtomicRmws => "atomic_rmws",
            Self::SharedReads => "shared_reads",
            Self::SharedWrites => "shared_writes",
            Self::KernelEntries => "kernel_entries",
            Self::Allocations => "allocations",
            Self::WorkingSet => "working_set",
        }
    }
}

/// Why an envelope computation has no result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvelopeError {
    /// The count of `quantity` does not fit `u64`.
    Overflow {
        /// The quantity whose count overflowed.
        quantity: Quantity,
    },
    /// More units than the cost's [`max_units`](crate::Cost::max_units) allow.
    AboveBound {
        /// Units asked for.
        units: u64,
        /// The bound.
        max: u64,
    },
    /// [`Cost::for_extent`](crate::Cost::for_extent) on an
    /// [`Item`](crate::Unit::Item)-unit cost: items are not derivable from bytes.
    NotAnExtentUnit,
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow { quantity } => write!(f, "{} overflows u64", quantity.name()),
            Self::AboveBound { units, max } => write!(f, "{units} units exceed the bound {max}"),
            Self::NotAnExtentUnit => f.write_str("items cannot be derived from a byte extent"),
        }
    }
}

impl core::error::Error for EnvelopeError {}

/// The first quantity found above its limit by
/// [`check_within`](ResourceEnvelope::check_within).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Exceeded {
    /// The quantity.
    pub quantity: Quantity,
    /// Its value.
    pub value: u64,
    /// The limit it exceeds.
    pub limit: u64,
}

impl fmt::Display for Exceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} = {} exceeds {}",
            self.quantity.name(),
            self.value,
            self.limit
        )
    }
}

impl core::error::Error for Exceeded {}

/// A count per [`Quantity`]. Plain data; the same type serves as a declared envelope, an
/// observed count, and an allowance.
///
/// ```
/// use o1_contract::{Quantity::*, ResourceEnvelope as E};
/// let a = E::ZERO.with(Ops, 1).with(BytesRead, 256).with(WorkingSet, 256);
/// let b = E::ZERO.with(Ops, 1).with(AtomicRmws, 1).with(WorkingSet, 64);
/// let seq = a.then(b).unwrap();           // b after a: totals add, peaks max
/// assert_eq!((seq.get(Ops), seq.get(WorkingSet)), (2, 256));
/// let par = a.alongside(b).unwrap();      // both live: peaks add too
/// assert_eq!(par.get(WorkingSet), 320);
/// assert_eq!(a.repeat(1000).unwrap().get(BytesRead), 256_000);
/// assert_eq!(a.repeat(1000).unwrap().get(WorkingSet), 256);
/// assert!(E::ZERO.with(Ops, u64::MAX).repeat(2).is_err()); // never wraps
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceEnvelope {
    q: [u64; Quantity::COUNT],
}

impl ResourceEnvelope {
    /// All quantities zero.
    pub const ZERO: Self = Self {
        q: [0; Quantity::COUNT],
    };

    /// This envelope with `quantity` set to `value`.
    #[inline]
    #[must_use]
    pub const fn with(self, quantity: Quantity, value: u64) -> Self {
        let mut q = self.q;
        q[quantity as usize] = value;
        Self { q }
    }

    /// The count of `quantity`.
    #[inline]
    #[must_use]
    pub const fn get(&self, quantity: Quantity) -> u64 {
        self.q[quantity as usize]
    }

    /// Whether every quantity is zero.
    #[must_use]
    pub const fn is_zero(&self) -> bool {
        let mut i = 0;
        while i < Quantity::COUNT {
            if self.q[i] != 0 {
                return false;
            }
            i += 1;
        }
        true
    }

    const fn combine(self, other: Self, peaks_add: bool) -> Result<Self, EnvelopeError> {
        let mut q = [0u64; Quantity::COUNT];
        let mut i = 0;
        while i < Quantity::COUNT {
            let quantity = Quantity::ALL[i];
            let (a, b) = (self.q[i], other.q[i]);
            q[i] = match quantity.kind() {
                Kind::Peak if !peaks_add => {
                    if a > b {
                        a
                    } else {
                        b
                    }
                }
                _ => match a.checked_add(b) {
                    Some(v) => v,
                    None => return Err(EnvelopeError::Overflow { quantity }),
                },
            };
            i += 1;
        }
        Ok(Self { q })
    }

    const fn scale(self, n: u64, peaks_scale: bool) -> Result<Self, EnvelopeError> {
        if n == 0 {
            return Ok(Self::ZERO);
        }
        let mut q = [0u64; Quantity::COUNT];
        let mut i = 0;
        while i < Quantity::COUNT {
            let quantity = Quantity::ALL[i];
            q[i] = match quantity.kind() {
                Kind::Peak if !peaks_scale => self.q[i],
                _ => match self.q[i].checked_mul(n) {
                    Some(v) => v,
                    None => return Err(EnvelopeError::Overflow { quantity }),
                },
            };
            i += 1;
        }
        Ok(Self { q })
    }

    /// `other` runs after `self`, and `self`'s footprint is no longer needed: totals add,
    /// peaks take the maximum.
    ///
    /// # Errors
    ///
    /// [`EnvelopeError::Overflow`] if a total does not fit `u64`.
    pub const fn then(self, other: Self) -> Result<Self, EnvelopeError> {
        self.combine(other, false)
    }

    /// `self` and `other` are live at the same time (parallel threads, or an interleaved
    /// loop): totals add and peaks add.
    ///
    /// # Errors
    ///
    /// [`EnvelopeError::Overflow`].
    pub const fn alongside(self, other: Self) -> Result<Self, EnvelopeError> {
        self.combine(other, true)
    }

    /// `n` executions over the same footprint: totals × `n`, peaks unchanged (`n ≥ 1`);
    /// `n = 0` is [`ZERO`](Self::ZERO).
    ///
    /// # Errors
    ///
    /// [`EnvelopeError::Overflow`].
    pub const fn repeat(self, n: u64) -> Result<Self, EnvelopeError> {
        self.scale(n, false)
    }

    /// `n` executions whose footprints all stay live: totals × `n` and peaks × `n`.
    ///
    /// # Errors
    ///
    /// [`EnvelopeError::Overflow`].
    pub const fn repeat_retained(self, n: u64) -> Result<Self, EnvelopeError> {
        self.scale(n, true)
    }

    /// The first quantity (in [`Quantity::ALL`] order) above `limit`.
    ///
    /// Used as `observed.check_within(&declared)` (a verifier) and
    /// `job.check_within(&allowance)` (a planner).
    ///
    /// # Errors
    ///
    /// [`Exceeded`] naming the quantity, its value and the limit.
    pub const fn check_within(&self, limit: &Self) -> Result<(), Exceeded> {
        let mut i = 0;
        while i < Quantity::COUNT {
            if self.q[i] > limit.q[i] {
                return Err(Exceeded {
                    quantity: Quantity::ALL[i],
                    value: self.q[i],
                    limit: limit.q[i],
                });
            }
            i += 1;
        }
        Ok(())
    }

    /// Every quantity `≤` the same quantity of `limit` (a partial order).
    #[must_use]
    pub const fn within(&self, limit: &Self) -> bool {
        self.check_within(limit).is_ok()
    }
}

impl Default for ResourceEnvelope {
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Debug for ResourceEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut m = f.debug_map();
        for q in Quantity::ALL {
            m.entry(&q.name(), &self.get(q));
        }
        m.finish()
    }
}

/// Non-zero quantities as `name=value`, comma-separated; `zero` if none.
impl fmt::Display for ResourceEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for q in Quantity::ALL {
            let v = self.get(q);
            if v != 0 {
                if !first {
                    f.write_str(", ")?;
                }
                write!(f, "{}={v}", q.name())?;
                first = false;
            }
        }
        if first {
            f.write_str("zero")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_is_in_discriminant_order() {
        for (i, q) in Quantity::ALL.iter().enumerate() {
            assert_eq!(*q as usize, i);
        }
    }

    #[test]
    fn only_working_set_is_a_peak() {
        for q in Quantity::ALL {
            assert_eq!(q.kind() == Kind::Peak, q == Quantity::WorkingSet);
        }
    }
}
