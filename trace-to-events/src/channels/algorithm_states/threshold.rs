use digital_muon_common::Intensity;
use crate::{
    channels::algorithm_states::AlgorithmState,
    parameters::FixedThresholdDiscriminatorParameters,
    pulse_detection::{EventsIterable, Real, threshold_detector::{ThresholdDetector, ThresholdDetectorParameters}}
};

/// Encapsulates all settings and objects in the differential threshold algorithm which persist across digitiser messages.
#[derive(Clone)]
pub(crate) struct ThresholdDetectorState {
    /// Parameters for the threshold detector.
    pub(crate) parameters: ThresholdDetectorParameters,
}

impl ThresholdDetectorState {
    pub(crate) fn new(parameters: &FixedThresholdDiscriminatorParameters) -> Self {
        Self {
            parameters: ThresholdDetectorParameters {
                threshold: parameters.threshold,
                duration: parameters.duration.into(),
                cool_off: parameters.cool_off.into(),
            },
        }
    }
}

impl AlgorithmState for ThresholdDetectorState {
    #[tracing::instrument(skip_all, level = "trace")]
    fn find_events(
        &mut self,
        trace: impl Clone + ExactSizeIterator<Item = Real> + DoubleEndedIterator,
        polarity_sign: Real,
        baseline: Real,
    ) -> (Vec<usize>, Vec<Intensity>) {
        let raw = (0..trace.len())
            .into_iter()
            .zip(trace
                .map(move |v|polarity_sign * (v as Real - baseline))
            );
        let pulses = 
        raw.clone().events(ThresholdDetector::new(&self.parameters));

        let mut index = Vec::<usize>::new();
        let mut voltage = Vec::<Intensity>::new();
        for pulse in pulses {
            index.push(pulse.0);
            voltage.push(pulse.1.pulse_height as Intensity);
        }
        (index, voltage)
    }
}
