use crate::engine::{
    FlattenableWithIndex, HasName, HasSource, Templates,
    values::{ConstantFilter, ValueError, ValueFilter},
};
use digital_muon_common::{Channel, DigitizerId, FrameNumber};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum CriteriaError {
    #[error("Channel conditions not found in instance, or source {0}")]
    NoChannels(String),
    #[error("Digitiser Id conditions not found in instance, or source {0}")]
    NoDigitiserIds(String),
    #[error("Period conditions not found in instance, or source {0}")]
    NoPeriods(String),
    #[error("Frame conditions not found in instance, or source {0}")]
    NoFrames(String),
    #[error("Value Error: {0}")]
    Value(#[from] ValueError),
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct CriteriaProperties {
    /// Filters by periods.
    pub(crate) periods: Option<ValueFilter<u64>>,
    /// Is applied to all voltages when traces are created
    pub(crate) frames: Option<ValueFilter<FrameNumber>>,
    /// Is applied to all voltages when traces are created
    pub(crate) channels: Option<ValueFilter<Channel>>,
    /// Is applied to all voltages when traces are created
    pub(crate) digitiser_ids: Option<ValueFilter<DigitizerId>>,
}

/// Defines a criteria template that can be used to construct a criteria object.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct CriteriaTemplate {
    pub(crate) name: String,
    #[serde(flatten)]
    pub(crate) properties: CriteriaProperties,
}

impl HasName for CriteriaTemplate {
    fn get_name(&self) -> &str {
        &self.name
    }
}

/// Encapsulates the critieria found in [BucketBlock].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Criteria {
    /// Refers to the [CriteriaTemplate] that can fill out any missing fields of [CriteriaProperties].
    pub(crate) source: String,
    /// Contains fields used in the crieria object, can be either specified here or in the [CriteriaTemplate] referred to by [Self::source].
    #[serde(flatten)]
    pub(crate) properties: CriteriaProperties,
}

impl HasSource for Criteria {
    fn get_source(&self) -> &str {
        &self.source
    }
}

/// Encapsulates crieria used in a [FlatBucket] object.
#[derive(Debug, Clone)]
pub(crate) struct FlatCriteria {
    /// Is applied to all voltages when traces are created
    pub(crate) periods: ConstantFilter<u64>,
    /// Is applied to all voltages when traces are created
    pub(crate) frames: ConstantFilter<FrameNumber>,
    /// Is applied to all voltages when traces are created
    pub(crate) channels: ConstantFilter<Channel>,
    /// Is applied to all voltages when traces are created
    pub(crate) digitiser_ids: ConstantFilter<DigitizerId>,
}

impl FlattenableWithIndex for Criteria {
    type Flat = FlatCriteria;
    type Library = Templates;
    type Error = CriteriaError;

    fn flatten(&self, libraries: &Templates, index: usize) -> Result<FlatCriteria, Self::Error> {
        let template = libraries.get_criteria(self.get_source());
        let periods = self
            .properties
            .periods
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.properties.periods.as_ref()))
            .map(|v| v.flatten(libraries.get_arrays(), index))
            .transpose()?
            .ok_or_else(|| CriteriaError::NoPeriods(self.get_source().into()))?;
        let frames = self
            .properties
            .frames
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.properties.frames.as_ref()))
            .map(|v| v.flatten(libraries.get_arrays(), index))
            .transpose()?
            .ok_or_else(|| CriteriaError::NoFrames(self.get_source().into()))?;
        let channels = self
            .properties
            .channels
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.properties.channels.as_ref()))
            .map(|v| v.flatten(libraries.get_arrays(), index))
            .transpose()?
            .ok_or_else(|| CriteriaError::NoChannels(self.get_source().into()))?;
        let digitiser_ids = self
            .properties
            .digitiser_ids
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.properties.digitiser_ids.as_ref()))
            .map(|v| v.flatten(libraries.get_arrays(), index))
            .transpose()?
            .ok_or_else(|| CriteriaError::NoDigitiserIds(self.get_source().into()))?;

        Ok(FlatCriteria {
            periods,
            frames,
            channels,
            digitiser_ids,
        })
    }
}
