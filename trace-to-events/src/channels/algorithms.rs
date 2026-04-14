//! Provides algorithm-specific functions and which extract and return lists of muon events using specified settings.
use core::f64;

use crate::{
    channels::{
        MultiscalingDetectorCache, PeakHeightParameters, SmoothingDetectorCache,
        algorithm_states::MultiscalingMethodAlgorithmState,
    },
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
        utils::{global_arg_min, std_dev},
        window::{FiniteDifferences, SliceWindow, convolution_filter::ConvolutionFilter},
    },
};
use digital_muon_common::{Intensity, Time};

/// Extract muon events from the given trace, using the fixed threshold discriminator and the given settings.
/// # Parameters
/// - trace: raw trace data.
/// - sample_time: sample time in ns.
/// - polarity_sign: the polarity of the trace signal.
/// - baseline: the baseline of the trace signal.
/// - parameters: settings to use for the fixed threshold discriminator.
#[tracing::instrument(skip_all, level = "trace")]
pub(super) fn find_fixed_threshold_events(
    trace: impl Iterator<Item = Real> + Clone,
    sample_time: Real,
    polarity_sign: Real,
    baseline: Real,
    parameters: &ThresholdDuration,
) -> (Vec<Time>, Vec<Intensity>) {
    let raw = trace.enumerate().map(|(i, v)| {
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
    trace: impl Iterator<Item = Real> + Clone,
    sample_time: Real,
    polarity_sign: Real,
    baseline: Real,
    finite_differences: &FiniteDifferences<2>,
    parameters: &DifferentialThresholdParameters,
    peak_height: &PeakHeightParameters,
) -> (Vec<Time>, Vec<Intensity>) {
    let raw = trace.enumerate().map(|(i, v)| {
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
#[tracing::instrument(skip_all, level = "trace", fields(std_dev, num_regions))]
pub(super) fn find_smoothing_events(
    trace: impl Clone + ExactSizeIterator<Item = Real> + DoubleEndedIterator,
    fin_diff_gaussian: &ConvolutionFilter,
    cache: &mut SmoothingDetectorCache,
    sample_time: Real,
    polarity_sign: Real,
    baseline: Real,
    parameters: &SmoothingDetectorParameters,
) -> (Vec<Time>, Vec<Intensity>) {
    cache.ensure_time_data_written(trace.len(), sample_time);
    // Get the radius of the kernel by right-bitshifting the size by one
    // i.e. divide by 2, and round-down.
    let kernel_radius = fin_diff_gaussian.kernel_size() >> 1;
    let padded = trace
        .clone()
        .map(|v| polarity_sign * (v as Real - baseline))
        .pad_reflect(kernel_radius, kernel_radius);
    cache.ensure_cache_lengths(trace.len() + fin_diff_gaussian.kernel_size(), trace.len());
    cache.write_input_values(padded);

    fin_diff_gaussian.apply_to_slice(
        cache.input_values.as_slice(),
        cache.output_values.as_mut_slice(),
    );

    let percentile = ((trace.len() as f64 * parameters.noise_centile) / 100.0) as usize;
    let noise_std = std_dev(&cache.output_values[percentile..])
        .expect("std_dev should exist, this should never fail.");
    tracing::Span::current().record("std_dev", noise_std);

    let output_iter = cache.output_values.iter().cloned().enumerate();

    let regions = output_iter
        .clone()
        .events(RegionDetector::new(
            -noise_std * parameters.nsig_noise,
            parameters.min_size,
        ))
        .collect::<Vec<_>>();
    tracing::Span::current().record("num_regions", regions.len());

    let pulses = regions
        .into_iter()
        .flat_map(|region| {
            let region_iter = output_iter.clone().take(region.1).skip(region.0);
            if let Some(use_local_for_sizes_ge) = parameters.use_local_for_sizes_ge
                && region_iter.len() >= use_local_for_sizes_ge
            {
                region_iter
                    .events(LocalArgMinDetector::default())
                    .collect::<Vec<_>>()
            } else {
                vec![global_arg_min(region_iter)]
            }
        })
        .collect::<Vec<_>>();

    let mut times = Vec::<Time>::new();
    let mut voltages = Vec::<Intensity>::new();
    for time in pulses {
        times.push(time as Time);
        voltages.push(trace.clone().nth(time).expect("") as Intensity);
    }
    (times, voltages)
}

/// FIXME
/// # Parameters
/// - trace: raw trace data.
/// - fin_diff_gaussian: the composite convolution filter applying the smoothing filer and taking the second derivative.
/// - cache: provides `Vec` objects which are used to write intermediate calculations.
/// - sample_time: sample time in ns.
/// - polarity_sign: the polarity of the trace signal.
/// - baseline: the baseline of the trace signal.
/// - parameters: settings to use for the smoothing detector.
#[tracing::instrument(skip_all, level = "trace")]
pub(super) fn find_multiscaling_events(
    trace: impl Clone + ExactSizeIterator<Item = Real> + DoubleEndedIterator,
    cache: &mut MultiscalingDetectorCache,
    sample_time: Real,
    polarity_sign: Real,
    baseline: Real,
    method_state: &mut MultiscalingMethodAlgorithmState,
) -> (Vec<Time>, Vec<Intensity>) {
    cache.ensure_cache_lengths(trace.len());
    cache.write_input_values(trace);

    let smoothed_trace = cache
        .pyramid
        .apply_to_slice(&cache.input_values)
        //.expect("Pyramid should be configured correctly, this should never fail.")
        .iter()
        .cloned();

    // Pass the smoothed trace on to the method.
    match method_state {
        MultiscalingMethodAlgorithmState::FixedThreshold { parameters } => {
            find_fixed_threshold_events(
                smoothed_trace,
                sample_time,
                polarity_sign,
                baseline,
                parameters,
            )
        }
        MultiscalingMethodAlgorithmState::DifferentialThreshold(state) => {
            find_differential_threshold_events(
                smoothed_trace,
                sample_time,
                polarity_sign,
                baseline,
                &state.finite_differences,
                &state.parameters,
                &state.peak_height,
            )
        }
        MultiscalingMethodAlgorithmState::Smoothing(state) => find_smoothing_events(
            smoothed_trace,
            &state.fin_diff_gaussian,
            &mut state.cache,
            sample_time,
            polarity_sign,
            baseline,
            &state.parameters,
        ),
    }
}
