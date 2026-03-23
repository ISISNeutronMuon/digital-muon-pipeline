//! Provides algorithm-specific functions and which extract and return lists of muon events using specified settings.
use crate::{
    channels::{PeakHeightParameters, SmoothingDetectorCache},
    parameters::{PeakHeightBasis, SmoothingDetectorParameters},
    pulse_detection::{
        EventsIterable, Real, WindowIterable,
        detectors::{
            differential_threshold_detector::{
                DifferentialThresholdDetector, DifferentialThresholdParameters,
            },
            local_arg_min_detector::LocalArgMinDetector,
            region_detector::RegionDetector,
        },
        iterators::PaddingIterable,
        threshold_detector::{ThresholdDetector, ThresholdDuration},
        utils::std_dev,
        window::{FiniteDifferences, SliceWindow, convolution_filter::ConvolutionFilter},
    },
};
use digital_muon_common::{Intensity, Time};
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::ChannelTrace;

/// Extract muon events from the given trace, using the fixed threshold discriminator and the given settings.
/// # Parameters
/// - trace: raw trace data.
/// - sample_time: sample time in ns.
/// - polarity_sign: the polarity of the trace signal.
/// - baseline: the baseline of the trace signal.
/// - parameters: settings to use for the fixed threshold discriminator.
#[tracing::instrument(skip_all, level = "trace")]
pub(super) fn find_fixed_threshold_events(
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
/// - polarity_sign: the polarity of the trace signal.
/// - baseline: the baseline of the trace signal.
/// - finite_differences: template `FiniteDifferences<2>` from which to efficiently clone.
/// - parameters: settings to use for the differential threshold detector.
/// - peak_height: settings determining how peak heights are calculated.
#[tracing::instrument(skip_all, level = "trace")]
pub(super) fn find_differential_threshold_events(
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

/// Extract muon events from the given trace, by first applying a smoothing filter,
/// taking the second derivative, and applying the `RegionDetector` and `LocalArgMinDetector`
/// in succession.
/// # Parameters
/// - trace: raw trace data.
/// - fin_diff_gaussian: the composite convolution filter applying the smoothing filer and taking the second derivative.
/// - cache: provides `Vec` objects which are used to write intermediate calculations.
/// - sample_time: sample time in ns.
/// - polarity_sign: the polarity of the trace signal.
/// - baseline: the baseline of the trace signal.
/// - parameters: settings to use for the smoothing detector.
#[tracing::instrument(skip_all, level = "trace")]
pub(super) fn find_smoothing_events(
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

    cache.ensure_time_data_written(raw_voltages.len(), sample_time);
    // Get the radius of the kernel by right-bitshifting the size by one
    // i.e. divide by 2, and round-down.
    let kernel_radius = fin_diff_gaussian.kernel_size() >> 1;
    let padded = raw_voltages
        .iter()
        .map(|v| polarity_sign * (v as Real - baseline))
        .pad_reflect(kernel_radius, kernel_radius);
    cache.ensure_cache_lengths(
        raw_voltages.len() + fin_diff_gaussian.kernel_size(),
        raw_voltages.len(),
    );
    cache.write_input_values(padded);

    fin_diff_gaussian.apply_to_slice(
        cache.input_values.as_slice(),
        cache.output_values.as_mut_slice(),
    );

    let percentile = ((raw_voltages.len() as f64 * parameters.noise_centile) / 100.0) as usize;
    let noise_std = std_dev(&cache.output_values[percentile..])
        .expect("StdDev should exist, this should never fail.");

    let output_iter = cache.output_values.iter().cloned().enumerate();

    let regions = output_iter.clone().events(RegionDetector::new(
        -noise_std * parameters.nsig_noise,
        parameters.min_size,
    ));
    let pulses = regions
        .flat_map(|region| {
            output_iter
                .clone()
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
    (times, voltages)
}
