use digital_muon_common::{Intensity, Time};

use crate::{channels::algorithm_states::AlgorithmState, parameters::FixedThresholdDiscriminatorParameters, pulse_detection::{EventsIterable, Real, threshold_detector::{ThresholdDetector, ThresholdDuration}}};


/// Encapsulates all settings and objects in the differential threshold algorithm which persist across digitiser messages.
#[derive(Clone)]
pub(crate) struct ThresholdDetectorState {
    /// Parameters for the threshold detector.
    pub(crate) parameters: ThresholdDuration,
}

impl ThresholdDetectorState {
    pub(crate) fn new(parameters: &FixedThresholdDiscriminatorParameters) -> Self {
        Self {
            parameters: ThresholdDuration {
                threshold: parameters.threshold,
                duration: parameters.duration,
                cool_off: parameters.cool_off,
            },
        }
    }
}

impl AlgorithmState for ThresholdDetectorState {
    #[tracing::instrument(skip_all, level = "trace")]
    fn find_events(
        &mut self,
        trace: impl Clone + ExactSizeIterator<Item = Real> + DoubleEndedIterator,
        sample_time: Real,
        polarity_sign: Real,
        baseline: Real,
    ) -> (Vec<Time>, Vec<Intensity>) {
        let raw = trace.enumerate().map(|(i, v)| {
            (
                i as Real * sample_time,
                polarity_sign * (v as Real - baseline),
            )
        });

        let pulses = raw.clone().events(ThresholdDetector::new(&self.parameters));

        let mut time = Vec::<Time>::new();
        let mut voltage = Vec::<Intensity>::new();
        for pulse in pulses {
            time.push(pulse.0 as Time);
            voltage.push(pulse.1.pulse_height as Intensity);
        }
        (time, voltage)
    }
}
