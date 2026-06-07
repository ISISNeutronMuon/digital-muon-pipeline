use std::ops::Deref;
use crate::engine::{
    Array, FlattenableWithIndex, HasName,
    values::{Value, ValueError},
};
use serde::Deserialize;

///
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Waveform {
    pub(crate) name: String,
    #[serde(flatten)]
    pub(crate) properties: WaveformProperties,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case", tag = "pulse-type")]
pub(crate) enum WaveformProperties {
    Flat { width: Value<f64> },
    Triangular { base_width: Value<f64> },
    Gaussian { sd: Value<f64> },
}

impl HasName for Waveform {
    fn get_name(&self) -> &str {
        &self.name
    }
}

impl Deref for Waveform {
    type Target = WaveformProperties;

    fn deref(&self) -> &Self::Target {
        &self.properties
    }
}

impl FlattenableWithIndex for WaveformProperties {
    type Flat = FlatWaveform;
    type Library = [Array];
    type Error = ValueError;

    fn flatten(&self, arrays: &Self::Library, index: usize) -> Result<FlatWaveform, Self::Error> {
        match self {
            Self::Flat { width } => Ok(FlatWaveform::Flat {
                width: width.flatten(arrays, index)?,
            }),
            Self::Triangular { base_width } => Ok(FlatWaveform::Triangular {
                base_width: base_width.flatten(arrays, index)?,
            }),
            Self::Gaussian { sd } => Ok(FlatWaveform::Gaussian {
                sd: sd.flatten(arrays, index)?,
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum FlatWaveform {
    Flat { width: f64 },
    Triangular { base_width: f64 },
    Gaussian { sd: f64 },
}
