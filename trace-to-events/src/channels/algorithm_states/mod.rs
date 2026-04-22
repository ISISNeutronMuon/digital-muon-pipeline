//! Provides objects for persisting state for a specific algorithm.
mod cache;
mod differential;
mod multiscaling;
mod smoothing;
mod threshold;

pub(crate) use cache::TimeCache;
pub(crate) use differential::DifferentialThresholdDiscriminatorState;
pub(crate) use multiscaling::{LayerProcessingSettings, MultiscalingDetectorState};
pub(crate) use smoothing::SmoothingDetectorState;
pub(crate) use threshold::ThresholdDetectorState;

use digital_muon_common::Intensity;
use crate::pulse_detection::Real;

pub(crate) trait AlgorithmState {
    /// Extract muon events from the given trace, using the fixed threshold discriminator and the given settings.
    /// Returns a pair of equally-sized vectors containing the index of the trace the event occurred at, and its
    /// corresponding intensity respectively.
    /// # Parameters
    /// - trace: raw trace data.
    /// - polarity_sign: the polarity of the trace signal.
    /// - baseline: the baseline of the trace signal.
    fn find_events(
        &mut self,
        trace: impl Clone + ExactSizeIterator<Item = Real> + DoubleEndedIterator,
        polarity_sign: Real,
        baseline: Real,
    ) -> (Vec<usize>, Vec<Intensity>);
}