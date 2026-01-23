//! Provides functions which extract and return lists of muon events using specified detectors and settings.
use crate::{
    parameters::{
        AdvancedMuonDetectorParameters, DetectorSettings,
        DifferentialThresholdDiscriminatorParameters, FixedThresholdDiscriminatorParameters, Mode,
        PeakHeightBasis, Polarity,
    },
    pulse_detection::{
        AssembleIterable, EventsIterable, Real, WindowIterable,
        advanced_muon_detector::{AdvancedMuonAssembler, AdvancedMuonDetector},
        detectors::differential_threshold_detector::{
            DifferentialThresholdDetector, DifferentialThresholdParameters,
        },
        threshold_detector::{ThresholdDetector, ThresholdDuration},
        window::{Baseline, FiniteDifferences, SmoothingWindow},
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
        Mode::AdvancedMuonDetector(parameters) => find_advanced_events(
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

/// Extract muon events from the given trace, using the advanced muon detector and the given settings.
/// # Parameters
/// - trace: raw trace data.
/// - sample_time: sample time in ns.
/// - polarity: the polarity of the trace signal.
/// - baseline: the baseline of the trace signal.
/// - parameters: settings to use for the advanced muon detector.
#[tracing::instrument(skip_all, level = "trace")]
fn find_advanced_events(
    trace: &ChannelTrace,
    sample_time: Real,
    polarity: &Polarity,
    baseline: Real,
    parameters: &AdvancedMuonDetectorParameters,
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

    let smoothed = raw
        .clone()
        .window(Baseline::new(parameters.baseline_length.unwrap_or(0), 0.1))
        .window(SmoothingWindow::new(
            parameters.smoothing_window_size.unwrap_or(1),
        ))
        .map(|(i, stats)| (i, stats.mean));

    let events = smoothed
        .clone()
        .window(FiniteDifferences::<2>::new())
        .events(AdvancedMuonDetector::new(
            parameters.muon_onset,
            parameters.muon_fall,
            parameters.muon_termination,
            parameters.duration,
        ));

    let pulses = events
        .clone()
        .assemble(AdvancedMuonAssembler::default())
        .filter(|pulse| {
            Option::zip(parameters.min_amplitude, pulse.peak.value)
                .map(|(min, val)| min <= val)
                .unwrap_or(true)
        })
        .filter(|pulse| {
            Option::zip(parameters.max_amplitude, pulse.peak.value)
                .map(|(max, val)| max >= val)
                .unwrap_or(true)
        });

    let mut time = Vec::<Time>::new();
    let mut voltage = Vec::<Intensity>::new();
    for pulse in pulses {
        time.push(pulse.steepest_rise.time.unwrap_or_default() as Time);
        voltage.push(pulse.peak.value.unwrap_or_default() as Intensity);
    }
    (time, voltage)
}
