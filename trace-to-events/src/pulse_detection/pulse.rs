//! Provides a general structure for capturing all possible attributes of a pulse.
//!
//! These attributes are optional, so that not all detectors/assemblers need to provide values for them.
use super::Real;
use super::RealArray;

/// A time-dependent value occuring in a trace.
#[derive(Default, Clone, Debug, PartialEq)]
pub(crate) struct TimeValue<T>
where
    T: Default + Clone,
{
    /// The time at which the value occurs.
    pub(crate) time: Real,
    /// The value of the trace.
    pub(crate) value: T,
}

/// A version of [TimeValue] in which the `time` or `value` field can be optional.
#[derive(Default, Clone, Debug)]
pub(crate) struct TimeValueOptional<T>
where
    T: Default + Clone,
{
    /// The time at which the value occurs.
    pub(crate) time: Option<Real>,
    /// The value of the trace.
    pub(crate) value: Option<T>,
}

impl<T> From<TimeValue<T>> for TimeValueOptional<T>
where
    T: Default + Clone + Copy,
{
    fn from(source: TimeValue<T>) -> Self {
        TimeValueOptional {
            time: Some(source.time),
            value: Some(source.value),
        }
    }
}

/// A general pulse.
///
/// This object is designed as a generic output for assemblers.
#[derive(Default)]
pub(crate) struct Pulse {
    /// Time at which the pulse starts, and the value at this time.
    #[allow(unused)] // TODO: Fixme
    pub(crate) start: TimeValueOptional<Real>,
    /// Time at which the pulse ends, and the value at this time.
    #[allow(unused)] // TODO: Fixme
    pub(crate) end: TimeValueOptional<Real>,
    /// Time at which the pulse peaks, and the value at this time.
    pub(crate) peak: TimeValueOptional<Real>,
    /// Time at which the pulse is rising most steeply, and the value and derivative at this time.
    pub(crate) steepest_rise: TimeValueOptional<RealArray<2>>,
    /// Time at which the pulse is falling most sharply, and the value and derivative at this time.
    #[allow(unused)] // TODO: Fixme
    pub(crate) sharpest_fall: TimeValueOptional<RealArray<2>>,
}
