use digital_muon_common::{Channel, DigitizerId, FrameNumber};
use serde::Deserialize;
use thiserror::Error;

use crate::engine::{
    FlattenableWithIndex, Templates,
    utils::WithSource,
    values::{ConstantFilter, ValueError, ValueFilter},
};

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
    #[error("Cannot find algorithm {0}.")]
    CannotFindAlgorithm(String),
    #[error("Cannot find waveform {0}.")]
    CannotFindWaveform(String),
    #[error("Value Error: {0}")]
    Value(#[from] ValueError),
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Criteria {
    /// Is applied to all voltages when traces are created
    pub(crate) periods: Option<ValueFilter<u64>>,
    /// Is applied to all voltages when traces are created
    pub(crate) frames: Option<ValueFilter<FrameNumber>>,
    /// Is applied to all voltages when traces are created
    pub(crate) channels: Option<ValueFilter<Channel>>,
    /// Is applied to all voltages when traces are created
    pub(crate) digitiser_ids: Option<ValueFilter<DigitizerId>>,
}

///
/// This struct is created from the configuration JSON file.
///
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

impl FlattenableWithIndex for WithSource<Criteria> {
    type Flat = FlatCriteria;
    type Library = Templates;
    type Error = CriteriaError;

    fn flatten(&self, libraries: &Templates, index: usize) -> Result<FlatCriteria, Self::Error> {
        let template: Option<&Criteria> = libraries.get_criteria(self.get_source());
        let periods = self
            .periods
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.periods.as_ref()))
            .map(|v| v.flatten(libraries.get_arrays(), index))
            .transpose()?
            .ok_or_else(|| CriteriaError::NoPeriods(self.get_source().into()))?;
        let frames = self
            .frames
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.frames.as_ref()))
            .map(|v| v.flatten(libraries.get_arrays(), index))
            .transpose()?
            .ok_or_else(|| CriteriaError::NoFrames(self.get_source().into()))?;
        let channels = self
            .channels
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.channels.as_ref()))
            .map(|v| v.flatten(libraries.get_arrays(), index))
            .transpose()?
            .ok_or_else(|| CriteriaError::NoChannels(self.get_source().into()))?;
        let digitiser_ids = self
            .digitiser_ids
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.digitiser_ids.as_ref()))
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
