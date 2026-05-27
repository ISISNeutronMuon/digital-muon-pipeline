use digital_muon_common::{Intensity, Time};
use serde::Deserialize;
use std::fmt::Debug;

use crate::engine::{Array, FlatWaveform, FlattenableWithIndex, values::Value};

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Algorithm {
    FixedThreshold {
        threshold: Value<f64>,
        duration: Value<Time>,
        cool_down: Value<Time>,
    }
}

impl FlattenableWithIndex for Algorithm {
    type Flat = FlatAlgorithm;
    type Library = [Array];
    type Error = String;

    fn flatten(&self, arrays: &[Array], index: usize) -> Result<FlatAlgorithm, Self::Error> {
        match self {
            Algorithm::FixedThreshold {
                threshold,
                duration,
                cool_down,
            } => {
                let threshold = threshold.flatten(arrays, index)?;
                let duration = duration.flatten(arrays, index)?;
                let cool_down = cool_down.flatten(arrays, index)?;
                Ok(FlatAlgorithm::FixedThreshold {
                    threshold,
                    duration,
                    cool_down,
                })
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum FlatAlgorithm {
    FixedThreshold {
        threshold: f64,
        duration: Time,
        cool_down: Time,
    },
}

impl FlatAlgorithm {
    pub(crate) fn is_true_positive(&self, waveform: &FlatWaveform, time: Time, intensity: Intensity, dist: i32) -> bool {
        match self {
            FlatAlgorithm::FixedThreshold { threshold, duration, cool_down } => {
                match waveform {
                    FlatWaveform::Flat { width } => (),
                    FlatWaveform::Triangular { base_width } => (),
                    FlatWaveform::Gaussian { sd } => (),
                };
            },
        }
        true
    }
}
