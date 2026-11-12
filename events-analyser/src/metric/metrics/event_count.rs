use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::engine::ValueError;
use crate::metric::{
    CompleteMetric, FlattenedMetric, JsonMetric, PartialMetric, error::MetricResultError,
};
use crate::metric::{FlatMetricShell, PartialMetricShell};

use crate::{
    eventlists::ChannelDataByTopic,
    metric::{
        outputs::PartialSeriesOutput,
        utils::{MeanSD, SumWithSumOfSqrs},
    },
};
use digital_muon_common::Channel;

/// Defines settings for the `EventCount` metric, that require flatteneing,
/// that is depend on other items defined in the configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct JsonEventCount {
    /// The topic name to listen to for eventlist messages.
    topic: String,
}

impl JsonMetric for JsonEventCount {
    type Flattened = FlatEventCount;
    type Settings = ();

    fn flatten_metric(&self, library: &[String]) -> Result<Self::Flattened, ValueError> {
        Ok(FlatEventCount {
            topic: library
                .iter()
                .enumerate()
                .find_map(|(index, this_topic)| (*this_topic == self.topic).then_some(index))
                .expect("This should never fail."),
        })
    }
}

/// Equivalent to settings in [JsonEventCount], after their dependencies have been resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatEventCount {
    /// The topic index to listen to for eventlist messages.
    topic: usize,
}

impl FlattenedMetric for FlatEventCount {
    type Json = JsonEventCount;
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PartialEventCount {
    count: HashMap<Channel, SumWithSumOfSqrs>,
}

impl PartialMetric for PartialEventCount {
    type Source = FlatEventCount;

    fn push(
        &mut self,
        source: &FlatMetricShell<Self::Source>,
        channel: Channel,
        collection_by_topic: &ChannelDataByTopic,
    ) {
        let data = collection_by_topic
            .get(source.flat.topic)
            .expect("Topic should exist, this should never fail.");
        self.count
            .entry(channel)
            .or_default()
            .add_to(data.get_time_intensity().len() as f64);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct CompletedEventCount {
    count: HashMap<Channel, MeanSD>,
    total_count: MeanSD,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum EventCountProperty {
    TotalMean,
    TotalMeanWithSd,
    ChannelsBoxPlot,
}

impl CompleteMetric for CompletedEventCount {
    type Partial = PartialEventCount;
    type Error = MetricResultError;
    type Property = EventCountProperty;
    type Output = PartialSeriesOutput;

    fn aggregate(source: &PartialMetricShell<Self::Partial>) -> Result<Self, MetricResultError> {
        let count = source
            .count
            .iter()
            .map(|(&key, sum)| (key, sum.mean_and_stddev()))
            .collect();
        let total_count = source
            .count
            .values()
            .fold(Default::default(), SumWithSumOfSqrs::compose_with)
            .mean_and_stddev();
        Ok(Self { count, total_count })
    }

    fn get_property(&self, property: Self::Property) -> Result<Self::Output, Self::Error> {
        match property {
            EventCountProperty::TotalMean => {
                Ok(PartialSeriesOutput::Value(Some(self.total_count.mean)))
            }
            EventCountProperty::TotalMeanWithSd => Ok(PartialSeriesOutput::ValueWithErrors(Some(
                (self.total_count.mean, self.total_count.sd),
            ))),
            EventCountProperty::ChannelsBoxPlot => Ok(PartialSeriesOutput::LabelledGroup(
                self.count
                    .iter()
                    .map(|(channel, stats)| (stats.mean, format!("Channel {channel}")))
                    .collect(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::event::ChannelData;

    #[test]
    fn test1() {
        let _true_data = ChannelData::new(vec![(49, 6), (55, 6), (77, 12)]);
        let _estimate_data = ChannelData::new(vec![
            (40, 6),
            (54, 6),
            (60, 12),
            (61, 12),
            (62, 12),
            (76, 12),
            (79, 12),
        ]);
        //FIXME
    }
}
