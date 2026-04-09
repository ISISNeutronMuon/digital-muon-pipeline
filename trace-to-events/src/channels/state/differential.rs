
use crate::{
    channels::algorithms::{
        find_differential_threshold_events, find_fixed_threshold_events, find_multiscaling_events,
        find_smoothing_events,
    },
    parameters::{
        DetectorSettings, DifferentialThresholdDiscriminatorParameters, Mode,
        MultiscalingDetectorParameters, PeakHeightBasis, PeakHeightMode, Polarity,
        SmoothingDetectorParameters,
    },
    pulse_detection::{
        Real,
        detectors::differential_threshold_detector::DifferentialThresholdParameters,
        threshold_detector::ThresholdDuration,
        window::{
            FiniteDifferences, SliceWindow, convolution_filter::{ConvolutionFilter, KernelType}, fft_inverse::FftInverse, pyramid::PyramidFilter
        },
    },
};
use digital_muon_common::{Intensity, Time};
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::ChannelTrace;
use num::{Complex, complex::ComplexFloat};

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
    pub(super) fn new(parameters: &DifferentialThresholdDiscriminatorParameters) -> Self {
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