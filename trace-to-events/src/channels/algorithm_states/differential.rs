//! Provides objects for persisting state for the differential detector algorithm.
use digital_muon_common::{Intensity, Time};

use crate::{
    channels::algorithm_states::AlgorithmState, parameters::{DifferentialThresholdDiscriminatorParameters, PeakHeightBasis, PeakHeightMode}, pulse_detection::{
        EventsIterable as _, Real, WindowIterable, detectors::differential_threshold_detector::{DifferentialThresholdDetector, DifferentialThresholdParameters}, window::FiniteDifferences
    }
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

impl AlgorithmState for DifferentialThresholdDiscriminatorState {
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

        let pulses = raw
            .clone()
            .window(self.finite_differences.clone_only_coefficients())
            .events(DifferentialThresholdDetector::new(
                &self.parameters,
                self.peak_height.mode.clone(),
            ));

        let mut time = Vec::<Time>::new();
        let mut voltage = Vec::<Intensity>::new();
        for pulse in pulses {
            time.push(pulse.0 as Time);
            voltage.push(match self.peak_height.basis {
                PeakHeightBasis::TraceBaseline => pulse.1.peak_height as Intensity,
                PeakHeightBasis::PulseBaseline => {
                    (pulse.1.peak_height - pulse.1.base_height) as Intensity
                }
            });
        }
        (time, voltage)
    }
}
