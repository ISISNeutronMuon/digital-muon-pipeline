//! Provides objects for persisting state algorithm-agnostic state.
use crate::{
    channels::algorithm_states::{
            AlgorithmState, DifferentialThresholdDiscriminatorState, MultiscalingDetectorState, SmoothingDetectorState, ThresholdDetectorState, TimeCache
        },
    parameters::{DetectorSettings, Mode, Polarity}, pulse_detection::Real,
};
use digital_muon_common::{Intensity, Time};
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::ChannelTrace;

/// Encapsulates settings and objects specific to an algorithm.
#[derive(Clone)]
enum ChannelAlgorithmState {
    /// Encapsulates channel state used by the Fixed Threshold algorithm.
    FixedThreshold(ThresholdDetectorState),
    /// Encapsulates channel state used by the Differential Threshold algorithm.
    DifferentialThreshold(DifferentialThresholdDiscriminatorState),
    /// Encapsulates channel state used by the Smoothing algorithm.
    Smoothing(SmoothingDetectorState),
    /// Encapsulates channel state used by the Smoothing algorithm.
    Multiscaling(MultiscalingDetectorState),
}

impl ChannelAlgorithmState {
    /// Creates a new `ChannelAlgorithmState` object defined from `mode`. The state object is specific to the detector chosen.
    /// # Parameters
    /// - mode: the `Mode` enum to create the state object from.
    pub(crate) fn new(mode: &Mode) -> Self {
        match mode {
            Mode::FixedThresholdDiscriminator(parameters) => Self::FixedThreshold(
                ThresholdDetectorState::new(parameters),
            ),
            Mode::DifferentialThresholdDiscriminator(parameters) => Self::DifferentialThreshold(
                DifferentialThresholdDiscriminatorState::new(parameters),
            ),
            Mode::SmoothingDetector(parameters) => {
                Self::Smoothing(SmoothingDetectorState::new(parameters))
            }
            Mode::Multiscaling(parameters) => {
                Self::Multiscaling(MultiscalingDetectorState::new(parameters))
            }
        }
    }
}

/// Encapsulates settings and objects for a channel which can be applied to each channel trace.
#[derive(Clone)]
pub(crate) struct ChannelState {
    /// The sign of the trace's polarity.
    polarity_sign: Real,
    /// The baseline of the trace signal.
    baseline: Real,
    ///
    time: TimeCache,
    /// The settings and objects specific to the algorithm used.
    algorithm: ChannelAlgorithmState,
}

impl ChannelState {
    /// Creates a new `ChannelState` object defined from `settings`.
    /// # Parameters
    /// - settings: the `DetectorSettings` to create the state object from.
    pub(crate) fn new(settings: &DetectorSettings) -> Self {
        let polarity_sign = match settings.polarity {
            Polarity::Positive => 1.0,
            Polarity::Negative => -1.0,
        };
        Self {
            polarity_sign,
            baseline: settings.baseline as Real,
            time: Default::default(),
            algorithm: ChannelAlgorithmState::new(settings.mode),
        }
    }

    /// Extract muon events from the given trace, using the given detector settings.
    /// # Parameters
    /// - trace: raw trace data.
    /// - sample_time: sample time in ns.
    /// - detector_settings: settings to use for the detector.
    #[tracing::instrument(skip_all, fields(channel = trace.channel(), num_pulses))]
    pub(crate) fn find_channel_events(
        &mut self,
        trace: &ChannelTrace,
        sample_time: Real,
    ) -> (Vec<Time>, Vec<Intensity>) {
        let trace = trace
            .voltage()
            .expect("Trace voltage should be Some, this should never fail.")
            .into_iter()
            .map(|x| x as Real);
        self.time.ensure_time_data_written(trace.len(), sample_time);
        let (indices, intensitices) = match &mut self.algorithm {
            ChannelAlgorithmState::FixedThreshold(state) => state.find_events(
                trace,
                self.polarity_sign,
                self.baseline
            ),
            ChannelAlgorithmState::DifferentialThreshold(state) => state.find_events(
                trace,
                self.polarity_sign,
                self.baseline
            ),
            ChannelAlgorithmState::Smoothing(state) => state.find_events(
                trace,
                self.polarity_sign,
                self.baseline
            ),
            ChannelAlgorithmState::Multiscaling(state) => state.find_events(
                trace,
                self.polarity_sign,
                self.baseline
            ),
        };
        tracing::Span::current().record("num_pulses", indices.len());
        let times = self.time.into_times(indices);
        (times, intensitices)
    }
}
