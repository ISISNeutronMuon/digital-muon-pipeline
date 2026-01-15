//! An abstraction of the time-dependent types that are processed by the various filters.
//!
//! [Todo] This modules can be combined with others for brevity
use super::{EventData, Temporal, TraceValue, eventdata::Empty};

/// Abstracts types that are processed by the various filters.
///
/// To implement TracePoint a type must contain time data and a value.
pub(crate) trait TracePoint: Clone {
    /// Represents the time of the data point.
    /// This should be trivially copyable (usually a scalar).
    type Time: Temporal;

    /// Represents the value of the data point.
    type Value: TraceValue;

    /// [Todo] Not needed.
    type Data: EventData;

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
    /// [Todo] Not needed.
    type Data = Empty;

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
