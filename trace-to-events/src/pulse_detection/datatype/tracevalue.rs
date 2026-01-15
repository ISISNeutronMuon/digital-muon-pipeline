//! An abstraction of the time-independent types that are processed by the various filters.
//!
//! [Todo] This modules can be combined with others for brevity
use super::Real;
use std::{
    fmt::{Debug, Display, Formatter, Result},
    ops::{Index, IndexMut},
};

/// Abstracts of the types that represent values processed by the various filters.
///
/// This differs from the TracePoint type in that TracePoint must represent a time value,
/// whereas TraceValue is time-agnostic.
pub(crate) trait TraceValue: Default + Clone + Debug + Display {
    /// The type which contains the value of the data point
    type ContentType: Default + Clone + Debug + Display;
}

impl TraceValue for Real {
    type ContentType = Real;
}

/// This type allows the use of static arrays of TraceValue types as TraceValues
/// that can be used in the pipeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TraceArray<const N: usize, T>(pub(crate) [T; N])
where
    T: TraceValue;

impl<const N: usize, T> TraceArray<N, T>
where
    T: TraceValue,
{
    pub(crate) fn new(value: [T; N]) -> Self {
        Self(value)
    }
}

impl<const N: usize, T> Default for TraceArray<N, T>
where
    T: TraceValue + Copy,
{
    fn default() -> Self {
        Self([T::default(); N])
    }
}

impl<const N: usize, T> Display for TraceArray<N, T>
where
    T: TraceValue,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let TraceArray(array) = self;
        for val in array.iter().take(N - 1) {
            write!(f, "{val},")?;
        }
        write!(f, "{0}", array[N - 1])
    }
}

impl<const N: usize, T> Index<usize> for TraceArray<N, T>
where
    T: TraceValue,
{
    type Output = T;

    fn index(&self, idx: usize) -> &T {
        &self.0[idx]
    }
}

impl<const N: usize, T> IndexMut<usize> for TraceArray<N, T>
where
    T: TraceValue,
{
    fn index_mut(&mut self, idx: usize) -> &mut T {
        &mut self.0[idx]
    }
}

impl<const N: usize, T: TraceValue + Copy> TraceValue for TraceArray<N, T> {
    type ContentType = TraceArray<N, T>;
}

/// In practice arrays of Real types are mostly used.
pub(crate) type RealArray<const N: usize> = TraceArray<N, Real>;

/// This type allows contains descriptive statistical data.
#[derive(Default, Clone, Debug)]
pub(crate) struct Stats {
    /// The current value.
    pub(crate) value: Real,
    /// The arithmetic mean.
    ///
    /// This may have been calculated from applying a window to a range of values.
    pub(crate) mean: Real,
    /// The variance.
    ///
    /// This may have been calculated from applying a window to a range of values.
    pub(crate) variance: Real,
}

impl From<Real> for Stats {
    fn from(value: Real) -> Self {
        Stats {
            value,
            mean: value,
            variance: 0.,
        }
    }
}

impl Display for Stats {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "({}, {}, {})", self.value, self.mean, self.variance)
    }
}

impl TraceValue for Stats {
    type ContentType = Stats;
}
