//! Provides functions and structs which extract and return lists of muon events using specified detectors and settings.
use crate::{
    parameters::{
        DetectorSettings, Mode, PeakHeightBasis, PeakHeightMode, Polarity,
        SmoothingDetectorParameters,
    },
    pulse_detection::{
        EventsIterable, Real, WindowIterable, detectors::{
            differential_threshold_detector::{
                DifferentialThresholdDetector, DifferentialThresholdParameters,
            }, local_arg_min_detector::LocalArgMinDetector, region_detector::RegionDetector
        }, iterators::PaddingIterable, threshold_detector::{ThresholdDetector, ThresholdDuration}, utils::stddev_from_slice, window::{FiniteDifferences, SliceWindow, convolution_filter::{ConvolutionFilter, KernelType}}
    },
};
use digital_muon_common::{Intensity, Time};
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::ChannelTrace;

/// Encapsulates settings to determine how peak heights should be calculated.
#[derive(Clone)]
struct PeakHeightParameters {
    /// Determines how the peak height is calculated.
    mode: PeakHeightMode,
    /// Determines the peak height baseline.
    basis: PeakHeightBasis,
}

/// Encapsulates all settings and objects in the differential threshold algorithm which persist across digitiser messages.
#[derive(Clone)]
struct DifferentialThresholdAlgorithmParameters {
    /// First Finite Difference Window.
    finite_differences: FiniteDifferences<2>,
    /// Parameters for the threshold detector.
    parameters: DifferentialThresholdParameters,
    /// Determines how the peak height is calculated.
    peak_height: PeakHeightParameters,
}

/// Encapsulates all settings and objects in the smoothing algorithm which persist across digitiser messages.
#[derive(Clone)]
struct SmoothingAlgorithmParameters {
    /// Parameters for the smoothing detector.
    parameters: SmoothingDetectorParameters,
    /// Gaussian Kernel.
    fin_diff_gaussian: ConvolutionFilter,
    /// This cache is persisted to avoid reallocations on every channel trace.
    cache: SmoothingDetectorCache,
}

/// Memory which is used in the smoothing algorithm.
/// These are persisted and overwritten each channel trace,
/// to avoid repeated memory reallocation.
#[derive(Default, Clone)]
struct SmoothingDetectorCache {
    time: Vec<Real>,
    input_values: Vec<Real>,
    input_values2: Vec<Real>,
    output_values: Vec<Real>,
}

impl SmoothingDetectorCache {
    /// Ensures the value caches are of sufficient length for the message.
    /// If the fields are too small, they are resized.
    /// # Parameters
    /// - size: the minimum length of the cache's vectors.
    fn ensure_cache_lengths(&mut self, input_size: usize, output_size: usize) {
        // FIXME: Should there be some sort of check for absurdly big trace sizes?
        if input_size > self.input_values.len() {
            self.input_values.resize(input_size, Default::default());
            self.input_values2.resize(input_size, Default::default());
        }
            
        if output_size > self.output_values.len() {
            self.output_values.resize(output_size, Default::default());
        }
    }

    /// Write to the `time` and `values` fields from an iterator over a pair of the time and trace values.
    ///
    /// This should not be called unless `Self::ensure_cache_lengths` has been called with the appropriate `size` value.
    /// # Parameters
    /// - raw: iterator from which the `time` and `values` fields are written.
    fn ensure_time_data_written(&mut self, time: impl Clone + ExactSizeIterator<Item = Real>) {
        if time.len() != self.time.len() {
            self.time = time.collect();
        }
    }

    /// Write to the `time` and `values` fields from an iterator over a pair of the time and trace values.
    ///
    /// This should not be called unless `Self::ensure_cache_lengths` has been called with the appropriate `size` value.
    /// # Parameters
    /// - raw: iterator from which the `time` and `values` fields are written.
    fn write_input_values(&mut self, raw: impl Iterator<Item = Real> + Clone) {
        for (i, v) in raw.enumerate() {
            self.input_values[i] = v;
        }
    }
}

/// Encapsulates settings and objects for the appropriate algorithm.
#[derive(Clone)]
enum ChannelAlgorithm {
    FixedThreshold { parameters: ThresholdDuration },
    DifferentialThreshold(DifferentialThresholdAlgorithmParameters),
    Smoothing(SmoothingAlgorithmParameters),
}

impl ChannelAlgorithm {
    pub(crate) fn new(mode: &Mode) -> Self {
        match mode {
            Mode::FixedThresholdDiscriminator(parameters) => Self::FixedThreshold {
                parameters: ThresholdDuration {
                    threshold: parameters.threshold,
                    duration: parameters.duration,
                    cool_off: parameters.cool_off,
                },
            },
            Mode::DifferentialThresholdDiscriminator(parameters) => {
                Self::DifferentialThreshold(DifferentialThresholdAlgorithmParameters {
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
                })
            }
            Mode::SmoothingDetector(parameters) => Self::Smoothing(SmoothingAlgorithmParameters {
                parameters: parameters.clone(),
                fin_diff_gaussian: ConvolutionFilter::new(KernelType::Composition {
                    left: Box::new(KernelType::FiniteDifference { order: 2 }),
                    right: Box::new(KernelType::Gaussian { sigma: parameters.kernel_sigma })
                }),
                cache: Default::default(),
            }),
        }
    }
}

/// Encapsulates settings and objects for a channel which can be applied to each channel trace.
#[derive(Clone)]
pub(crate) struct ChannelProcessor {
    polarity_sign: Real,
    baseline: Real,
    algorithm: ChannelAlgorithm,
}

impl ChannelProcessor {
    pub(crate) fn new(settings: &DetectorSettings) -> Self {
        let polarity_sign = match settings.polarity {
            Polarity::Positive => 1.0,
            Polarity::Negative => -1.0,
        };
        Self {
            polarity_sign,
            baseline: settings.baseline as Real,
            algorithm: ChannelAlgorithm::new(settings.mode),
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
        let result = match &mut self.algorithm {
            ChannelAlgorithm::FixedThreshold { parameters } => find_fixed_threshold_events(
                trace,
                sample_time,
                self.polarity_sign,
                self.baseline,
                parameters,
            ),
            ChannelAlgorithm::DifferentialThreshold(parameters) => {
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
            ChannelAlgorithm::Smoothing(parameters) => find_smoothing_events(
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

/// Extract muon events from the given trace, using the fixed threshold discriminator and the given settings.
/// # Parameters
/// - trace: raw trace data.
/// - sample_time: sample time in ns.
/// - polarity: the polarity of the trace signal.
/// - baseline: the baseline of the trace signal.
/// - parameters: settings to use for the fixed threshold discriminator.
#[tracing::instrument(skip_all, level = "trace")]
fn find_fixed_threshold_events(
    trace: &ChannelTrace,
    sample_time: Real,
    polarity_sign: Real,
    baseline: Real,
    parameters: &ThresholdDuration,
) -> (Vec<Time>, Vec<Intensity>) {
    let raw = trace
        .voltage()
        .unwrap()
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            (
                i as Real * sample_time,
                polarity_sign * (v as Real - baseline),
            )
        });

    let pulses = raw.clone().events(ThresholdDetector::new(parameters));

    let mut time = Vec::<Time>::new();
    let mut voltage = Vec::<Intensity>::new();
    for pulse in pulses {
        time.push(pulse.0 as Time);
        voltage.push(pulse.1.pulse_height as Intensity);
    }
    (time, voltage)
}

/// Extract muon events from the given trace, using the differential threshold detector and the given settings.
/// # Parameters
/// - trace: raw trace data.
/// - sample_time: sample time in ns.
/// - polarity: the polarity of the trace signal.
/// - baseline: the baseline of the trace signal.
/// - parameters: settings to use for the differential threshold detector.
#[tracing::instrument(skip_all, level = "trace")]
fn find_differential_threshold_events(
    trace: &ChannelTrace,
    sample_time: Real,
    polarity_sign: Real,
    baseline: Real,
    finite_differences: &FiniteDifferences<2>,
    parameters: &DifferentialThresholdParameters,
    peak_height: &PeakHeightParameters,
) -> (Vec<Time>, Vec<Intensity>) {
    let raw = trace
        .voltage()
        .unwrap()
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            (
                i as Real * sample_time,
                polarity_sign * (v as Real - baseline),
            )
        });

    let pulses = raw
        .clone()
        .window(finite_differences.clone_only_coefficients())
        .events(DifferentialThresholdDetector::new(
            parameters,
            peak_height.mode.clone(),
        ));

    let mut time = Vec::<Time>::new();
    let mut voltage = Vec::<Intensity>::new();
    for pulse in pulses {
        time.push(pulse.0 as Time);
        voltage.push(match peak_height.basis {
            PeakHeightBasis::TraceBaseline => pulse.1.peak_height as Intensity,
            PeakHeightBasis::PulseBaseline => {
                (pulse.1.peak_height - pulse.1.base_height) as Intensity
            }
        });
    }
    (time, voltage)
}

#[tracing::instrument(skip_all, level = "trace")]
fn find_smoothing_events(
    trace: &ChannelTrace,
    fin_diff_gaussian: &ConvolutionFilter,
    cache: &mut SmoothingDetectorCache,
    sample_time: Real,
    polarity_sign: Real,
    baseline: Real,
    parameters: &SmoothingDetectorParameters,
) -> (Vec<Time>, Vec<Intensity>) {
    let raw_voltages = trace
        .voltage()
        .expect("Trace voltage should be Some, this should never fail.");

    cache.ensure_time_data_written(
        (0..raw_voltages.len())
            .map(|t| (t as Real) * sample_time)
    );
    let kernel_radius = fin_diff_gaussian.kernel_size() >> 1;
    let padded = raw_voltages.iter()
        .map(|v|polarity_sign * (v as Real - baseline))
        .pad_reflect(kernel_radius, kernel_radius);
    cache.ensure_cache_lengths(raw_voltages.len() + fin_diff_gaussian.kernel_size(), raw_voltages.len());
    cache.write_input_values(padded);

    fin_diff_gaussian.apply_to_slice(cache.input_values.as_slice(), cache.output_values.as_mut_slice());
    //let mid_slice = &mut cache.input_values2[0..cache.input_values.len() - fin_diff.kernel_size()];
    //fin_diff.apply_to_slice(cache.input_values.as_slice(), mid_slice);
    //gaussian.apply_to_slice(mid_slice, cache.output_values.as_mut_slice());

    let percentile = ((raw_voltages.len() as f64 * parameters.noise_centile) / 100.0) as usize;
    let noise_std = stddev_from_slice(&cache.output_values[percentile..])
        .expect("StdDev should exist, this should never fail.");

    let output_iter = cache.output_values
        .iter()
        .cloned()
        .enumerate();

    let regions = output_iter.clone()
        .events(RegionDetector::new(
            -noise_std * parameters.nsig_noise,
            parameters.min_size,
        ));
    let pulses = regions
        .flat_map(|region| {
            output_iter.clone()
                .take(region.1)
                .skip(region.0)
                .events(LocalArgMinDetector::default())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut times = Vec::<Time>::new();
    let mut voltages = Vec::<Intensity>::new();
    for time in pulses {
        times.push(time as Time);
        voltages.push(raw_voltages.get(time));
    }
    (times,voltages)
}
