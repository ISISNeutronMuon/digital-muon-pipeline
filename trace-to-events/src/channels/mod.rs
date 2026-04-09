//! Provides functions and structs which extract and return lists of muon events using specified detectors and settings.
mod algorithm_states;
mod algorithms;
mod channel_state;

pub(crate) use algorithm_states::LayerProcessingSettings;
use algorithm_states::{MultiscalingDetectorCache, PeakHeightParameters, SmoothingDetectorCache};
pub(crate) use channel_state::ChannelState;
