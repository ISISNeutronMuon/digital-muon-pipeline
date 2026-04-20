//! Detectors are applied by [EventIter] iterators to a stream of trace inputs.
//! They register detections in the form of a stream of events.
pub mod differential_threshold_detector;
pub mod local_arg_min_detector;
pub mod region_detector;
pub mod threshold_detector;

use crate::pulse_detection::EventPoint;

use super::{EventData, Real, TracePoint};

/// Implement for detectors, which take in trace values and outputs events.
pub(crate) trait Detector: Default + Clone {
    /// Trace type for input.
    type TracePointType: TracePoint;
    /// Event type for output, this must have the same `Time` type as `TracePointType`.
    type EventOutputType : EventPoint;

    /// Takes in trace signals and possibly outputs an event.
    fn signal(
        &mut self,
        time: <Self::TracePointType as TracePoint>::Time,
        value: <Self::TracePointType as TracePoint>::Value,
    ) -> Option<Self::EventOutputType>;

    /// Call when the the trace signal has completed. If an event is in progress, it is dispatched.
    fn finish(&mut self) -> Option<Self::EventOutputType>;
}
