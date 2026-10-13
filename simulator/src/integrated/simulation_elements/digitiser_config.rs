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

/// Determines how the digitiser ids and channels are configured.
///
/// Should only be created via JSON deserialisation.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DigitiserConfig {
    /// Only the channels are calculated, ranging from `0` to `num_channels - 1`.
    #[serde(rename_all = "kebab-case")]
    AutoAggregatedFrame { num_channels: NumConstant<usize> },
    /// Only the channels are provided, taken from the given [Vec].
    #[serde(rename_all = "kebab-case")]
    ManualAggregatedFrame { channels: Vec<Channel> },
    /// Both the channels and digitiser ids are calculated, digitiser ids ranging from `0` to `num_digitisers - 1`,
    /// with `num_channels_per_digitiser` channels assigned sequentially.
    #[serde(rename_all = "kebab-case")]
    AutoDigitisers {
        num_digitisers: NumConstant<usize>,
        num_channels_per_digitiser: NumConstant<usize>,
    },
    /// All digitisers and channels are specified manually.
    #[serde(rename_all = "kebab-case")]
    ManualDigitisers(Vec<Digitiser>),
}

impl DigitiserConfig {
    /// Crates list of channels from the configuration.
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

    /// Crates list of [SimulationEngineDigitiser] from the configuration.
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

/// Configuration of a digitiser.
///
/// Should only be created via JSON deserialisation.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Digitiser {
    /// Digitiser id.
    pub(crate) id: DigitizerId,
    /// Interval of channels this digitiser should have.
    pub(crate) channels: Interval<Channel>,
}
