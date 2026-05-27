use crate::engine::{Array, FlattenableWithIndex, values::Value};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case", tag = "pulse-type")]
pub(crate) enum Waveform {
    Flat { width: Value<f64> },
    Triangular { base_width: Value<f64> },
    Gaussian { sd: Value<f64> },
}

impl FlattenableWithIndex for Waveform {
    type Flat = FlatWaveform;
    type Library = [Array];
    type Error = String;

    fn flatten(&self, arrays: &[Array], index: usize) -> Result<FlatWaveform, Self::Error> {
        match self {
            Waveform::Flat { width } => Ok(FlatWaveform::Flat {
                width: width.flatten(arrays, index)?,
            }),
            Waveform::Triangular { base_width } => Ok(FlatWaveform::Triangular {
                base_width: base_width.flatten(arrays, index)?,
            }),
            Waveform::Gaussian { sd } => Ok(FlatWaveform::Gaussian {
                sd: sd.flatten(arrays, index)?,
            }),
        }
    }
}

#[derive(Debug)]
pub(crate) enum FlatWaveform {
    Flat { width: f64 },
    Triangular { base_width: f64 },
    Gaussian { sd: f64 },
}
