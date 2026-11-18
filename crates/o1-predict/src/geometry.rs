//! Workload geometry (design §5): one typed parameter per curve. A leaf states one value (the
//! point on the curve); thread count is a key part, not geometry.

use core::fmt;

/// The parameter a curve is measured along.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Parameter {
    /// Bytes of one pass over a buffer (transform, write).
    ExtentBytes,
    /// Bytes of a reused working set (sweep, block order).
    WorkingSetBytes,
    /// Slots of a table (arena chase).
    Slots,
    /// Payload bytes per frame.
    PayloadBytes,
    /// Items (fill, drain, handoff).
    Items,
    /// Push+pop pairs.
    Pairs,
    /// Operations per sample (atomics, spawns).
    Ops,
    /// Operations per thread (two-thread counters).
    OpsPerThread,
    /// Bytes of a resident table (lookups).
    TableBytes,
}

impl Parameter {
    /// `snake_case` name.
    pub fn name(self) -> &'static str {
        match self {
            Self::ExtentBytes => "extent_bytes",
            Self::WorkingSetBytes => "working_set_bytes",
            Self::Slots => "slots",
            Self::PayloadBytes => "payload_bytes",
            Self::Items => "items",
            Self::Pairs => "pairs",
            Self::Ops => "ops",
            Self::OpsPerThread => "ops_per_thread",
            Self::TableBytes => "table_bytes",
        }
    }

    /// Every parameter.
    pub const ALL: [Parameter; 9] = [
        Self::ExtentBytes,
        Self::WorkingSetBytes,
        Self::Slots,
        Self::PayloadBytes,
        Self::Items,
        Self::Pairs,
        Self::Ops,
        Self::OpsPerThread,
        Self::TableBytes,
    ];
}

/// Why a geometry or a curve is not valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryError {
    /// A value of zero: no curve has a point there and no leaf does zero-sized work this way.
    Zero,
    /// A curve's points are not strictly increasing.
    NotIncreasing,
    /// A curve has no point.
    Empty,
}

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Zero => "a geometry value is zero",
            Self::NotIncreasing => "curve points are not strictly increasing",
            Self::Empty => "a curve has no point",
        })
    }
}

/// One point on a curve: the parameter and its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "GeometryForm"))]
pub struct Geometry {
    /// The parameter.
    pub parameter: Parameter,
    /// Its value (non-zero).
    pub value: u64,
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct GeometryForm {
    parameter: Parameter,
    value: u64,
}

#[cfg(feature = "serde")]
impl TryFrom<GeometryForm> for Geometry {
    type Error = GeometryError;

    fn try_from(f: GeometryForm) -> Result<Self, GeometryError> {
        Geometry::new(f.parameter, f.value)
    }
}

impl Geometry {
    /// A geometry.
    ///
    /// # Errors
    ///
    /// [`GeometryError::Zero`].
    pub fn new(parameter: Parameter, value: u64) -> Result<Self, GeometryError> {
        if value == 0 {
            return Err(GeometryError::Zero);
        }
        Ok(Self { parameter, value })
    }
}

/// Validates curve points: non-empty, non-zero, strictly increasing.
///
/// # Errors
///
/// [`GeometryError`].
pub fn validate_points(points: &[u64]) -> Result<(), GeometryError> {
    if points.is_empty() {
        return Err(GeometryError::Empty);
    }
    if points[0] == 0 {
        return Err(GeometryError::Zero);
    }
    if points.windows(2).any(|w| w[0] >= w[1]) {
        return Err(GeometryError::NotIncreasing);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_refused() {
        assert_eq!(
            Geometry::new(Parameter::ExtentBytes, 0),
            Err(GeometryError::Zero)
        );
        assert!(Geometry::new(Parameter::ExtentBytes, 1).is_ok());
        assert!(Geometry::new(Parameter::TableBytes, u64::MAX).is_ok());
    }

    #[test]
    fn curve_points_must_increase() {
        assert_eq!(validate_points(&[]), Err(GeometryError::Empty));
        assert_eq!(validate_points(&[0, 4]), Err(GeometryError::Zero));
        assert_eq!(validate_points(&[4, 4]), Err(GeometryError::NotIncreasing));
        assert_eq!(validate_points(&[8, 4]), Err(GeometryError::NotIncreasing));
        assert_eq!(validate_points(&[1, 2, 1 << 40]), Ok(()));
    }

    #[test]
    fn names_are_distinct() {
        for (i, a) in Parameter::ALL.iter().enumerate() {
            for b in &Parameter::ALL[i + 1..] {
                assert_ne!(a.name(), b.name());
            }
        }
    }
}
