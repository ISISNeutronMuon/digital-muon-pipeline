use digital_muon_common::Time;
use serde::Deserialize;

use crate::engine::{Array, FlattenableWithIndex, values::Value};


#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct AlgorithmTemplate {
    name: String,
    value: Algorithm,
}

impl AlgorithmTemplate {
    pub(crate) fn has_name(&self, name: &str) -> bool {
        self.name == name
    }

    pub(crate) fn get_algorithm(&self) -> &Algorithm {
        &self.value
    }
}


#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Algorithm {
    FixedThreshold{
        threshold: Value<f64>,
        duration: Value<Time>,
        cool_down: Value<Time>,
    },
}

impl FlattenableWithIndex for Algorithm {
    type Flat = FlatAlgorithm;
    type Library = [Array];
    type Error = String;

    fn flatten(&self, arrays: &[Array], index: usize) -> Result<FlatAlgorithm, Self::Error> {
        match self {
            Algorithm::FixedThreshold { threshold, duration, cool_down } => {
                let threshold = threshold.flatten(arrays, index)?;
                let duration = duration.flatten(arrays, index)?;
                let cool_down = cool_down.flatten(arrays, index)?;
                Ok(FlatAlgorithm::FixedThreshold { threshold, duration, cool_down })
            },
        }

    }
}

#[derive(Debug)]
pub(crate) enum FlatAlgorithm {
    FixedThreshold{
        threshold: f64,
        duration: Time,
        cool_down: Time,
    },
}