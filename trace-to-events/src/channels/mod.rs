//! Provides functions and structs which extract and return lists of muon events using specified detectors and settings.
mod algorithms;
mod state;

pub(crate) use state::ChannelState;
use state::{MultiscalingDetectorCache, PeakHeightParameters, SmoothingDetectorCache};
