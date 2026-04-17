use crate::{parameters::FixedThresholdDiscriminatorParameters, pulse_detection::threshold_detector::ThresholdDuration};


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
