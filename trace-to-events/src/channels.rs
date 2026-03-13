//! Provides functions which extract and return lists of muon events using specified detectors and settings.
use crate::{
    parameters::{
        DetectorSettings, DifferentialThresholdDiscriminatorParameters,
        FixedThresholdDiscriminatorParameters, Mode, PeakHeightBasis, Polarity,
        SmoothingDetectorParameters,
    },
    pulse_detection::{
        EventsIterable, Real, WindowIterable,
        detectors::{
            differential_threshold_detector::{
                DifferentialThresholdDetector, DifferentialThresholdParameters,
            },
            smoothing_detector::sec_deriv_smoothing_for_peaks,
        },
        threshold_detector::{ThresholdDetector, ThresholdDuration},
        window::FiniteDifferences,
    },
};
use digital_muon_common::{Intensity, Time};
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::ChannelTrace;

/// Extract muon events from the given trace, using the given detector settings.
/// # Parameters
/// - trace: raw trace data.
/// - sample_time: sample time in ns.
/// - detector_settings: settings to use for the detector.
#[tracing::instrument(skip_all, fields(channel = trace.channel(), num_pulses))]
pub(crate) fn find_channel_events(
    trace: &ChannelTrace,
    sample_time: Real,
    detector_settings: &DetectorSettings,
) -> (Vec<Time>, Vec<Intensity>) {
    let result = match &detector_settings.mode {
        Mode::FixedThresholdDiscriminator(parameters) => find_fixed_threshold_events(
            trace,
            sample_time,
            detector_settings.polarity,
            detector_settings.baseline as Real,
            parameters,
        ),
        Mode::DifferentialThresholdDiscriminator(parameters) => find_differential_threshold_events(
            trace,
            sample_time,
            detector_settings.polarity,
            detector_settings.baseline as Real,
            parameters,
        ),
        Mode::SmoothingDetector(parameters) => find_smoothing_events(
            trace,
            sample_time,
            detector_settings.polarity,
            detector_settings.baseline as Real,
            parameters,
        ),
    };
    tracing::Span::current().record("num_pulses", result.0.len());
    result
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
    polarity: &Polarity,
    baseline: Real,
    parameters: &FixedThresholdDiscriminatorParameters,
) -> (Vec<Time>, Vec<Intensity>) {
    let sign = match polarity {
        Polarity::Positive => 1.0,
        Polarity::Negative => -1.0,
    };
    let raw = trace
        .voltage()
        .unwrap()
        .into_iter()
        .enumerate()
        .map(|(i, v)| (i as Real * sample_time, sign * (v as Real - baseline)));

    let pulses = raw
        .clone()
        .events(ThresholdDetector::new(&ThresholdDuration {
            threshold: parameters.threshold,
            duration: parameters.duration,
            cool_off: parameters.cool_off,
        }));

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
    polarity: &Polarity,
    baseline: Real,
    parameters: &DifferentialThresholdDiscriminatorParameters,
) -> (Vec<Time>, Vec<Intensity>) {
    let sign = match polarity {
        Polarity::Positive => 1.0,
        Polarity::Negative => -1.0,
    };
    let raw = trace
        .voltage()
        .unwrap()
        .into_iter()
        .enumerate()
        .map(|(i, v)| (i as Real * sample_time, sign * (v as Real - baseline)));

    let pulses = raw.clone().window(FiniteDifferences::<2>::new()).events(
        DifferentialThresholdDetector::new(
            &DifferentialThresholdParameters {
                begin_threshold: parameters.begin_threshold,
                begin_duration: parameters.begin_duration.into(),
                end_threshold: parameters.end_threshold,
                end_duration: parameters.end_duration.into(),
                cool_off: parameters.cool_off.into(),
            },
            parameters.peak_height_mode.clone(),
        ),
    );

    let mut time = Vec::<Time>::new();
    let mut voltage = Vec::<Intensity>::new();
    for pulse in pulses {
        time.push(pulse.0 as Time);
        voltage.push(match parameters.peak_height_basis {
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
    sample_time: Real,
    polarity: &Polarity,
    baseline: Real,
    parameters: &SmoothingDetectorParameters,
) -> (Vec<Time>, Vec<Intensity>) {
    let sign = match polarity {
        Polarity::Positive => 1.0,
        Polarity::Negative => -1.0,
    };
    let raw = trace
        .voltage()
        .unwrap()
        .into_iter()
        .map(|v| sign * (v as Real - baseline))
        .collect::<Vec<Real>>();
    let time = (0..raw.len())
        .map(|t| t as Real * sample_time)
        .collect::<Vec<Real>>();

    let (time, intensity) = sec_deriv_smoothing_for_peaks(
        &time,
        &raw,
        parameters.noise_centile,
        parameters.kernel_sigma,
        parameters.nsig_noise,
        parameters.min_size,
    )
    .unwrap();
    (
        time.into_iter().map(|t| t as Time).collect(),
        intensity.into_iter().map(|v| v as Intensity).collect(),
    )
}
