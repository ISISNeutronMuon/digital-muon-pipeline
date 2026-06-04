use digital_muon_common::{Intensity, Time};
use serde::Deserialize;
use std::{fmt::Debug, ops::Deref};

use crate::engine::{
    Array, FlatWaveform, FlattenableWithIndex,
    utils::HasName,
    values::{Value, ValueError},
};

///
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Algorithm {
    pub(crate) name: String,
    #[serde(flatten)]
    pub(crate) properties: AlgorithmProperties,
}

impl HasName for Algorithm {
    fn get_name(&self) -> &str {
        &self.name
    }
}

impl Deref for Algorithm {
    type Target = AlgorithmProperties;

    fn deref(&self) -> &Self::Target {
        &self.properties
    }
}

///
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AlgorithmProperties {
    ///
    FixedThreshold {
        ///
        threshold: Value<f64>,
        ///
        duration: Value<Time>,
        ///
        cool_down: Value<Time>,
    },
}

impl FlattenableWithIndex for AlgorithmProperties {
    type Flat = FlatAlgorithm;
    type Library = [Array];
    type Error = ValueError;

    fn flatten(&self, arrays: &Self::Library, index: usize) -> Result<FlatAlgorithm, Self::Error> {
        match self {
            Self::FixedThreshold {
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

#[derive(Debug, Clone)]
pub(crate) enum FlatAlgorithm {
    FixedThreshold {
        threshold: f64,
        duration: Time,
        cool_down: Time,
    },
}

impl FlatAlgorithm {
    pub(crate) fn is_true_positive(
        &self,
        waveform: &FlatWaveform,
        intensity: Intensity,
        dist: u32,
    ) -> bool {
        match self {
            &FlatAlgorithm::FixedThreshold {
                threshold,
                duration,
                cool_down,
            } => {
                let _height = threshold / intensity as f64;
                let width = match waveform {
                    &FlatWaveform::Flat { width } => width,
                    &FlatWaveform::Triangular { base_width } => base_width,
                    &FlatWaveform::Gaussian { sd } => sd,
                };
                (dist as f64) < (duration + cool_down) as f64 + width
            }
        }
    }
}
