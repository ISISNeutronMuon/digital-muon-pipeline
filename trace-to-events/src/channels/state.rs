use crate::{
    channels::algorithms::{
        find_differential_threshold_events, find_fixed_threshold_events, find_smoothing_events,
    },
    parameters::{
        DetectorSettings, DifferentialThresholdDiscriminatorParameters, Mode, PeakHeightBasis,
        PeakHeightMode, Polarity, SmoothingDetectorParameters,
    },
    pulse_detection::{
        Real,
        detectors::differential_threshold_detector::DifferentialThresholdParameters,
        threshold_detector::ThresholdDuration,
        window::{
            FiniteDifferences,
            convolution_filter::{ConvolutionFilter, KernelType},
        },
    },
};
use digital_muon_common::{Intensity, Time};
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::ChannelTrace;

/// Encapsulates settings to determine how peak heights should be calculated.
#[derive(Clone)]
pub(super) struct PeakHeightParameters {
    /// Determines how the peak height is calculated.
    pub(super) mode: PeakHeightMode,
    /// Determines the peak height baseline.
    pub(super) basis: PeakHeightBasis,
}

/// Encapsulates all settings and objects in the differential threshold algorithm which persist across digitiser messages.
#[derive(Clone)]
pub(super) struct DifferentialThresholdDiscriminatorState {
    /// First Finite Difference Window.
    finite_differences: FiniteDifferences<2>,
    /// Parameters for the threshold detector.
    parameters: DifferentialThresholdParameters,
    /// Determines how the peak height is calculated.
    peak_height: PeakHeightParameters,
}

impl DifferentialThresholdDiscriminatorState {
    fn new(parameters: &DifferentialThresholdDiscriminatorParameters) -> Self {
        Self {
            finite_differences: FiniteDifferences::<2>::new(),
            parameters: DifferentialThresholdParameters {
                begin_threshold: parameters.begin_threshold,
                begin_duration: parameters.begin_duration.into(),
                end_threshold: parameters.end_threshold,
                end_duration: parameters.end_duration.into(),
                cool_off: parameters.cool_off.into(),
            },
            peak_height: PeakHeightParameters {
                mode: parameters.peak_height_mode.clone(),
                basis: parameters.peak_height_basis.clone(),
            },
        }
    }
}

/// Encapsulates all settings and objects in the smoothing algorithm which persist across digitiser messages.
#[derive(Clone)]
pub(super) struct SmoothingDetectorState {
    /// Parameters for the smoothing detector.
    parameters: SmoothingDetectorParameters,
    /// Composite Kernel uses to smooth the trace and take the second derivative.
    fin_diff_gaussian: ConvolutionFilter,
    /// This cache is persisted to avoid reallocations on every channel trace.
    cache: SmoothingDetectorCache,
}

impl SmoothingDetectorState {
    fn new(parameters: &SmoothingDetectorParameters) -> Self {
        Self {
            parameters: parameters.clone(),
            fin_diff_gaussian: ConvolutionFilter::new(KernelType::Composition {
                left: Box::new(KernelType::FiniteDifference { order: 2 }),
                right: Box::new(KernelType::Gaussian {
                    sigma: parameters.kernel_sigma,
                }),
            }),
            cache: Default::default(),
        }
    }
}

/// Memory which is used in the smoothing algorithm.
/// These are persisted and overwritten each channel trace,
/// to avoid repeated memory reallocation.
#[derive(Default, Clone)]
pub(super) struct SmoothingDetectorCache {
    /// Value of `sample_time`
    expected_sample_time: Option<Real>,
    /// Memory in which to write the time bin values.
    time: Vec<Real>,
    /// Memory in which to write the pre-convolution trace data.
    pub(super) input_values: Vec<Real>,
    /// Memory in which the convolution window should write its output.
    pub(super) output_values: Vec<Real>,
}

impl SmoothingDetectorCache {
    /// Refreshes the `time` vector if and only if the size of the vector changes, or the `sample_time` field.
    /// # Parameters
    /// - size: the intended size of the `time` vector.
    /// - sample_time: the intended `sample_time`, defining the scale of the time-series.
    pub(super) fn ensure_time_data_written(&mut self, size: usize, sample_time: Real) {
        if size != self.time.len()
            || self
                .expected_sample_time
                .is_some_and(|current_sample_time| current_sample_time != sample_time)
        {
            self.time = (0..size).map(|t| t as Real * sample_time).collect();
            self.expected_sample_time = Some(sample_time);
        }
    }

    /// Ensures the value caches are of sufficient length for the message.
    /// If the fields are too small, they are resized.
    /// # Parameters
    /// - size: the minimum length of the cache's vectors.
    pub(super) fn ensure_cache_lengths(&mut self, input_size: usize, output_size: usize) {
        // FIXME: Should there be some sort of check for absurdly big trace sizes?
        if input_size > self.input_values.len() {
            self.input_values.resize(input_size, Default::default());
        }

        if output_size > self.output_values.len() {
            self.output_values.resize(output_size, Default::default());
        }
    }

    /// Write to the `input_values` field from an iterator over the appropriately padded trace values.
    ///
    /// This should not be called unless `Self::ensure_cache_lengths` has been called with the appropriate `size` value.
    /// # Parameters
    /// - input: iterator from which the `input_values` field is written.
    pub(super) fn write_input_values(&mut self, input: impl Iterator<Item = Real> + Clone) {
        for (i, v) in input.enumerate() {
            self.input_values[i] = v;
        }
    }
}

/// Encapsulates settings and objects specific to an algorithm.
#[derive(Clone)]
enum ChannelAlgorithmState {
    /// Encapsulates channel state used by the Fixed Threshold algorithm.
    FixedThreshold { parameters: ThresholdDuration },
    /// Encapsulates channel state used by the Differential Threshold algorithm.
    DifferentialThreshold(DifferentialThresholdDiscriminatorState),
    /// Encapsulates channel state used by the Smoothing algorithm.
    Smoothing(SmoothingDetectorState),
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
        };
        tracing::Span::current().record("num_pulses", result.0.len());
        result
    }
}
