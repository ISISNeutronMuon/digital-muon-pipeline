//! Provides a general structure for capturing all possible attributes of a pulse.
//! 
//! These attributes are optional, so that not all detectors/assemblers need to provide values for them.
use std::fmt::Display;

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

impl<T> Display for TimeValue<T>
where
    T: Default + Clone + Copy + Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{0},{1}", self.time, self.value))
    }
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
    T: Default + Clone + Copy + Display,
{
    fn from(source: TimeValue<T>) -> Self {
        TimeValueOptional {
            time: Some(source.time),
            value: Some(source.value),
        }
    }
}

impl<T> Display for TimeValueOptional<T>
where
    T: Default + Clone + Copy + Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{0},{1}",
            self.time.unwrap_or_default(),
            self.value.unwrap_or_default()
        ))
    }
}

/// A general pulse.
/// 
/// This object is designed as a generic output for assemblers.
#[derive(Default)]
pub(crate) struct Pulse {
    /// Time at which the pulse starts, and the value at this time.
    pub(crate) start: TimeValueOptional<Real>,
    /// Time at which the pulse ends, and the value at this time.
    pub(crate) end: TimeValueOptional<Real>,
    /// Time at which the pulse peaks, and the value at this time.
    pub(crate) peak: TimeValueOptional<Real>,
    /// Time at which the pulse is rising most steeply, and the value and derivative at this time.
    pub(crate) steepest_rise: TimeValueOptional<RealArray<2>>,
    /// Time at which the pulse is falling most sharply, and the value and derivative at this time.
    pub(crate) sharpest_fall: TimeValueOptional<RealArray<2>>,
}

impl Display for Pulse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{0},{1},{2},{3},{4}",
            self.start, self.end, self.peak, self.steepest_rise, self.sharpest_fall
        ))
    }
}
