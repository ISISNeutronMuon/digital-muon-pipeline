//! Provides objects for persisting state algorithm-agnostic state.
use crate::{
    channels::{
        algorithm_states::{
            DifferentialThresholdDiscriminatorState, MultiscalingDetectorState,
            SmoothingDetectorState,
        },
        algorithms::{
            find_differential_threshold_events, find_fixed_threshold_events,
            find_multiscaling_events, find_smoothing_events,
        },
    },
    parameters::{DetectorSettings, Mode, Polarity},
    pulse_detection::{Real, threshold_detector::ThresholdDuration},
};
use digital_muon_common::{Intensity, Time};
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::ChannelTrace;

/// Encapsulates settings and objects specific to an algorithm.
#[derive(Clone)]
enum ChannelAlgorithmState {
    /// Encapsulates channel state used by the Fixed Threshold algorithm.
    FixedThreshold { parameters: ThresholdDuration },
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
            Mode::FixedThresholdDiscriminator(parameters) => Self::FixedThreshold {
                parameters: ThresholdDuration {
                    threshold: parameters.threshold,
                    duration: parameters.duration,
                    cool_off: parameters.cool_off,
                },
            },
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
        let result = match &mut self.algorithm {
            ChannelAlgorithmState::FixedThreshold { parameters } => find_fixed_threshold_events(
                trace,
                sample_time,
                self.polarity_sign,
                self.baseline,
                parameters,
            ),
            ChannelAlgorithmState::DifferentialThreshold(parameters) => {
                find_differential_threshold_events(
                    trace,
                    sample_time,
                    self.polarity_sign,
                    self.baseline,
                    &parameters.finite_differences,
                    &parameters.parameters,
                    &parameters.peak_height,
                )
            }
            ChannelAlgorithmState::Smoothing(parameters) => find_smoothing_events(
                trace,
                &parameters.fin_diff_gaussian,
                &mut parameters.cache,
                sample_time,
                self.polarity_sign,
                self.baseline,
                &parameters.parameters,
            ),
            ChannelAlgorithmState::Multiscaling(state) => find_multiscaling_events(
                trace,
                &mut state.cache,
                &state.downsample_smoothing,
                &state.upsample_smoothing,
                sample_time,
                self.polarity_sign,
                self.baseline,
                &mut state.method_state,
            ),
        };
        tracing::Span::current().record("num_pulses", result.0.len());
        result
    }
}
