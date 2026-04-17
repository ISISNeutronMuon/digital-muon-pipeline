//! Provides objects for persisting state for a specific algorithm.
mod differential;
mod multiscaling;
mod smoothing;
mod threshold;

pub(crate) use differential::DifferentialThresholdDiscriminatorState;
use digital_muon_common::{Intensity, Time};
pub(crate) use multiscaling::{LayerProcessingSettings, MultiscalingDetectorState};
pub(crate) use smoothing::SmoothingDetectorState;
pub(crate) use threshold::ThresholdDetectorState;

use crate::pulse_detection::Real;

pub(crate) trait AlgorithmState {
    /// Extract muon events from the given trace, using the fixed threshold discriminator and the given settings.
    /// # Parameters
    /// - trace: raw trace data.
    /// - sample_time: sample time in ns.
    /// - polarity_sign: the polarity of the trace signal.
    /// - baseline: the baseline of the trace signal.
    fn find_events(
        &mut self,
        trace: impl Clone + ExactSizeIterator<Item = Real> + DoubleEndedIterator,
        sample_time: Real,
        polarity_sign: Real,
        baseline: Real,
    ) -> (Vec<Time>, Vec<Intensity>);
}