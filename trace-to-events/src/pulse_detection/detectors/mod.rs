//! Detectors are applied by [EventIter] iterators to a stream of trace inputs.
//! They register detections in the form of a stream of events.
pub mod advanced_muon_detector;
pub mod differential_threshold_detector;
pub mod threshold_detector;

use super::{EventData, EventPoint, Pulse, Real, RealArray, TracePoint, pulse::TimeValue};

/// Implement for detectors, which take in trace values and outputs events.
pub(crate) trait Detector: Default + Clone {
    /// Trace type for input.
    type TracePointType: TracePoint;
    /// Event type for output, this must have the same `Time` type as `TracePointType`.
    type EventPointType: EventPoint<TimeType = <Self::TracePointType as TracePoint>::Time>;

    /// Takes in trace signals and possibly outputs an event.
    fn signal(
        &mut self,
        time: <Self::TracePointType as TracePoint>::Time,
        value: <Self::TracePointType as TracePoint>::Value,
    ) -> Option<Self::EventPointType>;

    /// Call when the the trace signal has completed. If an event is in progress, it is dispatched.
    fn finish(&mut self) -> Option<Self::EventPointType>;
}

/// Implement for assemblers, which take in a list of events and outputs pulses.
pub(crate) trait Assembler: Default + Clone {
    /// The detector the assembler follows from.
    type DetectorType: Detector;

    /// Takes in a detector event and possibly outputs a pulse.
    /// # Parameters
    fn assemble_pulses(
        &mut self,
        source: <Self::DetectorType as Detector>::EventPointType,
    ) -> Option<Pulse>;
}
