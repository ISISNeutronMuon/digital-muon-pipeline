//! An abstraction of the time-independent types that are processed by the various filters.
use super::{Real, Temporal};
use std::{
    fmt::Debug,
    ops::{Index, IndexMut},
};

/// Abstracts of the types that represent values processed by the various filters.
///
/// This differs from the TracePoint type in that TracePoint must represent a time value,
/// whereas TraceValue is time-agnostic.
pub(crate) trait TraceValue: Default + Clone + Debug {
    /// The type which contains the value of the data point
    type ContentType: Default + Clone + Debug;
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

impl TraceValue for Stats {
    type ContentType = Stats;
}

/// Abstracts types that are processed by the various filters.
///
/// To implement TracePoint a type must contain time data and a value.
pub(crate) trait TracePoint: Clone {
    /// Represents the time of the data point.
    /// This should be trivially copyable (usually a scalar).
    type Time: Temporal;

    /// Represents the value of the data point.
    type Value: TraceValue;

    /// Returns the time of the data point.
    fn get_time(&self) -> Self::Time;

    /// Returns an immutable reference to the value of the data point.
    fn get_value(&self) -> &Self::Value;

    /// Take ownership of a clone of the value without destructing the data point.
    fn clone_value(&self) -> Self::Value {
        self.get_value().clone()
    }
}

/// This is the most basic non-trivial TraceData type.
/// The first element is the TimeType and the second the ValueType.
/// feedback.
impl<X, Y> TracePoint for (X, Y)
where
    X: Temporal,
    Y: TraceValue,
{
    type Time = X;
    type Value = Y;

    fn get_time(&self) -> Self::Time {
        self.0
    }

    fn get_value(&self) -> &Self::Value {
        &self.1
    }

    fn clone_value(&self) -> Self::Value {
        self.get_value().clone()
    }
}
