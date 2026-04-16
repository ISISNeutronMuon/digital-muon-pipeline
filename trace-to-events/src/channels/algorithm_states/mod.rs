//! Provides objects for persisting state for a specific algorithm.
mod differential;
mod multiscaling;
mod smoothing;

pub(crate) use differential::{DifferentialThresholdDiscriminatorState, PeakHeightParameters};
pub(crate) use multiscaling::{
    LayerProcessingSettings, MultiscalingDetectorCache, MultiscalingDetectorState,
    MultiscalingMethodAlgorithmState,
};
pub(crate) use smoothing::{SmoothingDetectorCache, SmoothingDetectorState};
