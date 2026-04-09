use crate::{
    parameters::{DifferentialThresholdDiscriminatorParameters, PeakHeightBasis, PeakHeightMode},
    pulse_detection::{
        detectors::differential_threshold_detector::DifferentialThresholdParameters,
        window::FiniteDifferences,
    },
};
/// Encapsulates settings to determine how peak heights should be calculated.
#[derive(Clone)]
pub(crate) struct PeakHeightParameters {
    /// Determines how the peak height is calculated.
    pub(crate) mode: PeakHeightMode,
    /// Determines the peak height baseline.
    pub(crate) basis: PeakHeightBasis,
}

/// Encapsulates all settings and objects in the differential threshold algorithm which persist across digitiser messages.
#[derive(Clone)]
pub(crate) struct DifferentialThresholdDiscriminatorState {
    /// First Finite Difference Window.
    pub(crate) finite_differences: FiniteDifferences<2>,
    /// Parameters for the threshold detector.
    pub(crate) parameters: DifferentialThresholdParameters,
    /// Determines how the peak height is calculated.
    pub(crate) peak_height: PeakHeightParameters,
}

impl DifferentialThresholdDiscriminatorState {
    pub(crate) fn new(parameters: &DifferentialThresholdDiscriminatorParameters) -> Self {
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
