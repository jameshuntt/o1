//! Cost shape (`fixed + units × per_unit`) and the size conversions that produce `units`.

use core::num::NonZeroU64;

use crate::envelope::{EnvelopeError, ResourceEnvelope};

/// What one unit of size is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    /// An item (a record, an element). Not derivable from a byte extent.
    Item,
    /// A byte.
    Byte,
    /// A block of `bytes` bytes.
    Block {
        /// Block size.
        bytes: NonZeroU64,
    },
    /// A cache line of `bytes` bytes (the line size is a parameter, never a constant).
    Line {
        /// Line size.
        bytes: NonZeroU64,
    },
}

/// Whether the per-unit working set grows with the number of units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeakGrowth {
    /// Each unit's footprint is released (or is the same footprint) before the next: the
    /// per-unit peak counts once.
    Fixed,
    /// Every unit's footprint stays live: the per-unit peak is multiplied by the units.
    PerUnit,
}

/// `fixed + units × per_unit` for one execution of an operation.
///
/// ```
/// use core::num::NonZeroU64;
/// use o1_contract::{Cost, PeakGrowth, Quantity::*, ResourceEnvelope as E, Unit};
/// // A 256-byte block primitive over a 4 MiB job: 16 384 executions.
/// let block = NonZeroU64::new(256).unwrap();
/// let cost = Cost {
///     fixed: E::ZERO.with(Ops, 1).with(WorkingSet, 4096), // setup + hot state
///     per_unit: E::ZERO.with(Ops, 1).with(BytesRead, 256).with(BytesWritten, 256)
///         .with(WorkingSet, 512),
///     unit: Unit::Block { bytes: block },
///     max_units: None,
///     peak_growth: PeakGrowth::Fixed,
/// };
/// let job = cost.for_extent(0, 4 << 20).unwrap();
/// assert_eq!(job.get(Ops), 1 + 16_384);
/// assert_eq!(job.get(BytesRead), 4 << 20);
/// assert_eq!(job.get(WorkingSet), 4096 + 512);
/// // One byte more needs one more (full-block upper-bound) execution.
/// assert_eq!(cost.for_extent(0, (4 << 20) + 1).unwrap().get(Ops), 1 + 16_385);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cost {
    /// Once per execution: setup, or the whole of an O(1) operation.
    pub fixed: ResourceEnvelope,
    /// Per unit of size.
    pub per_unit: ResourceEnvelope,
    /// What a unit is.
    pub unit: Unit,
    /// A fixed bound on units (a type capacity or a buffer length), if any.
    pub max_units: Option<u64>,
    /// Whether the per-unit peak grows with units.
    pub peak_growth: PeakGrowth,
}

impl Cost {
    /// The cost of an O(1) operation: `envelope` per execution, nothing per unit.
    #[must_use]
    pub const fn constant(envelope: ResourceEnvelope) -> Self {
        Self {
            fixed: envelope,
            per_unit: ResourceEnvelope::ZERO,
            unit: Unit::Item,
            max_units: None,
            peak_growth: PeakGrowth::Fixed,
        }
    }

    /// The envelope of one execution over `units` units:
    /// `fixed.alongside(per_unit.repeat(units))` (or `repeat_retained` for
    /// [`PeakGrowth::PerUnit`]). The fixed part is combined with `alongside` because setup
    /// state stays live through the units.
    ///
    /// # Errors
    ///
    /// [`EnvelopeError::AboveBound`] if `units > max_units`; [`EnvelopeError::Overflow`].
    pub const fn at(&self, units: u64) -> Result<ResourceEnvelope, EnvelopeError> {
        if let Some(max) = self.max_units {
            if units > max {
                return Err(EnvelopeError::AboveBound { units, max });
            }
        }
        let scaled = match self.peak_growth {
            PeakGrowth::Fixed => self.per_unit.repeat(units),
            PeakGrowth::PerUnit => self.per_unit.repeat_retained(units),
        };
        match scaled {
            Ok(s) => self.fixed.alongside(s),
            Err(e) => Err(e),
        }
    }

    /// The units an extent of `len` bytes starting `offset` bytes into a line needs:
    /// [`Unit::Byte`] → `len`; [`Unit::Block`] → [`covering`](Blocks::covering) blocks
    /// (the remainder costed as one more full block — an upper bound; `offset` does not
    /// matter, blocks are measured from the extent's start); [`Unit::Line`] →
    /// [`lines_touched`].
    ///
    /// # Errors
    ///
    /// [`EnvelopeError::NotAnExtentUnit`] for [`Unit::Item`].
    pub const fn units_for_extent(&self, offset: u64, len: u64) -> Result<u64, EnvelopeError> {
        match self.unit {
            Unit::Item => Err(EnvelopeError::NotAnExtentUnit),
            Unit::Byte => Ok(len),
            Unit::Block { bytes } => Ok(split_blocks(len, bytes).covering()),
            Unit::Line { bytes } => Ok(lines_touched(offset, len, bytes)),
        }
    }

    /// [`at`](Self::at)`(`[`units_for_extent`](Self::units_for_extent)`(offset, len))`.
    ///
    /// # Errors
    ///
    /// As those two.
    pub const fn for_extent(
        &self,
        offset: u64,
        len: u64,
    ) -> Result<ResourceEnvelope, EnvelopeError> {
        match self.units_for_extent(offset, len) {
            Ok(units) => self.at(units),
            Err(e) => Err(e),
        }
    }
}

/// `len` bytes cut into blocks: `full` whole blocks and a `remainder` (`< block`) of bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Blocks {
    /// Whole blocks.
    pub full: u64,
    /// Bytes after the last whole block (`0..block`).
    pub remainder: u64,
}

impl Blocks {
    /// Blocks needed to cover every byte: `full`, plus one if there is a remainder. Never
    /// overflows (`full ≤ u64::MAX / 2` whenever `remainder > 0`).
    #[inline]
    #[must_use]
    pub const fn covering(&self) -> u64 {
        self.full + (self.remainder > 0) as u64
    }
}

/// `len` bytes in blocks of `block`: `full = len / block`, `remainder = len % block`.
/// Total; the remainder is reported, never dropped.
#[inline]
#[must_use]
pub const fn split_blocks(len: u64, block: NonZeroU64) -> Blocks {
    Blocks {
        full: len / block.get(),
        remainder: len % block.get(),
    }
}

/// Cache lines of `line` bytes touched by `len` bytes starting `offset` bytes after a line
/// boundary (only `offset % line` matters). `0` for `len = 0`.
///
/// `ceil((offset % line + len) / line)`, computed in `u128`, so it cannot overflow;
/// the result is at most `len` for `line = 1` and below `len / 2 + 2` otherwise, so it
/// always fits `u64`.
///
/// ```
/// use core::num::NonZeroU64;
/// use o1_contract::lines_touched;
/// let line = NonZeroU64::new(64).unwrap();
/// assert_eq!(lines_touched(0, 64, line), 1);
/// assert_eq!(lines_touched(1, 64, line), 2);   // unaligned start: one more line
/// assert_eq!(lines_touched(63, 2, line), 2);
/// assert_eq!(lines_touched(5, 0, line), 0);
/// ```
#[inline]
#[must_use]
pub const fn lines_touched(offset: u64, len: u64, line: NonZeroU64) -> u64 {
    if len == 0 {
        return 0;
    }
    let line = line.get() as u128;
    let start = offset as u128 % line;
    (start + len as u128).div_ceil(line) as u64
}
