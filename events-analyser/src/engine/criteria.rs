use digital_muon_common::{
    Channel, DigitizerId, FrameNumber
};
use serde::Deserialize;

use crate::engine::{FlattenableWithIndex, Templates, values::{ConstantFilter, ValueFilter}};

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Criteria {
    // Is applied to all voltages when traces are created
    pub(crate) name: String,
    // Is applied to all voltages when traces are created
    pub(crate) periods: Option<ValueFilter<u64>>,
    // Is applied to all voltages when traces are created
    pub(crate) frames: Option<ValueFilter<FrameNumber>>,
    // Is applied to all voltages when traces are created
    pub(crate) channels: Option<ValueFilter<Channel>>,
    // Is applied to all voltages when traces are created
    pub(crate) digitiser_ids: Option<ValueFilter<DigitizerId>>,
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug)]
pub(crate) struct FlatCriteria {
    // Is applied to all voltages when traces are created
    pub(crate) periods: ConstantFilter<u64>,
    // Is applied to all voltages when traces are created
    pub(crate) frames: ConstantFilter<FrameNumber>,
    // Is applied to all voltages when traces are created
    pub(crate) channels: ConstantFilter<Channel>,
    // Is applied to all voltages when traces are created
    pub(crate) digitiser_ids: ConstantFilter<DigitizerId>,
}

impl FlattenableWithIndex for Criteria {
    type Flat = FlatCriteria;
    type Library = Templates;
    type Error = String;

    fn flatten(&self, libraries: &Templates, index: usize) -> Result<FlatCriteria, Self::Error> {
        let template: Option<&Criteria> = libraries.get_criteria().iter().find(|tmplt|tmplt.name == self.name);
        let periods = self.periods.as_ref()
            .or_else(||template.and_then(|tmplt|tmplt.periods.as_ref()))
            .map(|v|v.flatten(libraries.get_arrays(), index))
            .transpose()?;
        let frames = self.frames.as_ref()
            .or_else(||template.and_then(|tmplt|tmplt.frames.as_ref()))
            .map(|v|v.flatten(libraries.get_arrays(), index))
            .transpose()?;
        let channels = self.channels.as_ref()
            .or_else(||template.and_then(|tmplt|tmplt.channels.as_ref()))
            .map(|v|v.flatten(libraries.get_arrays(), index))
            .transpose()?;
        let digitiser_ids = self.digitiser_ids.as_ref()
            .or_else(||template.and_then(|tmplt|tmplt.digitiser_ids.as_ref()))
            .map(|v|v.flatten(libraries.get_arrays(), index))
            .transpose()?;

        periods
            .zip(frames)
            .zip(channels)
            .zip(digitiser_ids)
            .map(|(((periods, frames), channels), digitiser_ids)|
                FlatCriteria { periods, frames, channels, digitiser_ids }
            ).ok_or_else(||format!("Could not construct criteria."))
    }
}