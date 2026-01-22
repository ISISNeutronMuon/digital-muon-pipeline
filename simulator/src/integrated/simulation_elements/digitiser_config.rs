use crate::integrated::{
    simulation_elements::{
        Interval,
        utils::{JsonValueError, NumConstant},
    },
    simulation_engine::engine::SimulationEngineDigitiser,
};
use digital_muon_common::{Channel, DigitizerId};
use serde::Deserialize;
use tracing::instrument;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DigitiserConfig {
    #[serde(rename_all = "kebab-case")]
    AutoAggregatedFrame { num_channels: NumConstant<usize> },
    #[serde(rename_all = "kebab-case")]
    ManualAggregatedFrame { channels: Vec<Channel> },
    #[serde(rename_all = "kebab-case")]
    AutoDigitisers {
        num_digitisers: NumConstant<usize>,
        num_channels_per_digitiser: NumConstant<usize>,
    },
    #[serde(rename_all = "kebab-case")]
    ManualDigitisers(Vec<Digitiser>),
}

impl DigitiserConfig {
    #[instrument(skip_all)]
    pub(crate) fn generate_channels(&self) -> Result<Vec<Channel>, JsonValueError> {
        let channels = match self {
            DigitiserConfig::AutoAggregatedFrame { num_channels } => {
                (0..num_channels.value()? as Channel).collect()
            }
            DigitiserConfig::ManualAggregatedFrame { channels } => channels.clone(),
            DigitiserConfig::AutoDigitisers {
                num_digitisers,
                num_channels_per_digitiser,
            } => (0..((num_digitisers.value()? * num_channels_per_digitiser.value()?) as Channel))
                .collect(),
            DigitiserConfig::ManualDigitisers(digitisers) => digitisers
                .iter()
                .flat_map(|digitiser| digitiser.channels.range_inclusive())
                .collect(),
        };
        Ok(channels)
    }

    #[instrument(skip_all)]
    pub(crate) fn generate_digitisers(
        &self,
    ) -> Result<Vec<SimulationEngineDigitiser>, JsonValueError> {
        let digitisers = match self {
            DigitiserConfig::AutoAggregatedFrame { .. } => Default::default(),
            DigitiserConfig::ManualAggregatedFrame { .. } => Default::default(),
            DigitiserConfig::AutoDigitisers {
                num_digitisers,
                num_channels_per_digitiser,
            } => (0..num_digitisers.value()?)
                .map(|d| {
                    Ok(SimulationEngineDigitiser::new(
                        d as DigitizerId,
                        ((d * num_channels_per_digitiser.value()?)
                            ..((d + 1) * num_channels_per_digitiser.value()?))
                            .collect(),
                    ))
                })
                .collect::<Result<_, JsonValueError>>()?,
            DigitiserConfig::ManualDigitisers(digitisers) => digitisers
                .iter()
                .map(|digitiser| SimulationEngineDigitiser {
                    id: digitiser.id,
                    channel_indices: Vec::<_>::new(), //TODO
                })
                .collect(),
        };
        Ok(digitisers)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Digitiser {
    pub(crate) id: DigitizerId,
    pub(crate) channels: Interval<Channel>,
}
