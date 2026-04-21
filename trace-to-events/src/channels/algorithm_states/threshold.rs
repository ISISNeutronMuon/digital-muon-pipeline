use digital_muon_common::{Intensity, Time};

use crate::{
    channels::algorithm_states::{AlgorithmState, AlgorithmStateIterator},
    parameters::FixedThresholdDiscriminatorParameters,
    pulse_detection::{EventsIterable, Real, threshold_detector::{ThresholdDetector, ThresholdDetectorParameters}}
};

/// Memory which is used in the smoothing algorithm.
/// These are persisted and overwritten each channel trace,
/// to avoid repeated memory reallocation.
#[derive(Default, Clone)]
pub(crate) struct ThresholdDetectorCache {
    /// Value of `sample_time`
    expected_sample_time: Option<Real>,
    /// Memory in which to write the time bin values.
    time: Vec<Real>,
}

impl ThresholdDetectorCache {
    /// Refreshes the `time` vector if and only if the size of the vector changes, or the `sample_time` field.
    /// # Parameters
    /// - size: the intended size of the `time` vector.
    /// - sample_time: the intended `sample_time`, defining the scale of the time-series.
    pub(crate) fn ensure_time_data_written(&mut self, size: usize, sample_time: Real) {
        if size != self.time.len()
            || self
                .expected_sample_time
                .is_some_and(|current_sample_time| current_sample_time != sample_time)
        {
            self.time = (0..size).map(|t| t as Real * sample_time).collect();
            self.expected_sample_time = Some(sample_time);
        }
    }
}


/// Encapsulates all settings and objects in the differential threshold algorithm which persist across digitiser messages.
#[derive(Clone)]
pub(crate) struct ThresholdDetectorState {
    /// Parameters for the threshold detector.
    pub(crate) parameters: ThresholdDetectorParameters,
    /// This cache is persisted to avoid reallocations on every channel trace.
    pub(crate) cache: ThresholdDetectorCache,
}

impl ThresholdDetectorState {
    pub(crate) fn new(parameters: &FixedThresholdDiscriminatorParameters) -> Self {
        Self {
            parameters: ThresholdDetectorParameters {
                threshold: parameters.threshold,
                duration: parameters.duration.into(),
                cool_off: parameters.cool_off.into(),
            },
            cache: Default::default(),
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
        self.cache.ensure_time_data_written(trace.len(), sample_time);
        let raw = (0..self.cache.time.len())
            .into_iter()
            //.cloned()
            .zip(trace
                .map(|v|polarity_sign * (v as Real - baseline))
            );

        let pulses = raw.clone().events(ThresholdDetector::new(&self.parameters));

        let mut time = Vec::<Time>::new();
        let mut voltage = Vec::<Intensity>::new();
        for pulse in pulses {
            time.push(*self.cache.time.get(pulse.0).expect("") as Time);
            voltage.push(pulse.1.pulse_height as Intensity);
        }
        (time, voltage)
    }
}
