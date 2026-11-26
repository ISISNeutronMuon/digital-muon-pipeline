use crate::{
    engine::ValueError,
    eventlists::ChannelDataByTopic,
    metric::{
        CompleteMetric, DependentMetric, PartialMetric, PartialMetricShell, ResolvedMetric,
        ResolvedMetricShell,
        error::MetricResultError,
        outputs::PartialSeriesOutput,
        utils::{MeanSD, SumWithSumOfSqrs},
    },
};
use digital_muon_common::Channel;
use digital_muon_streaming_types::FrameMetadata;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Defines settings for the `EventCount` metric, that require resolving,
/// that is depend on other items defined in the configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct DependentEventCount {
    /// The topic name to listen to for eventlist messages.
    topic: String,
}

impl DependentMetric for DependentEventCount {
    type Resolved = ResolvedEventCount;
    type Independent = ();

    fn resolve_metric(&self, library: &[String]) -> Result<Self::Resolved, ValueError> {
        Ok(ResolvedEventCount {
            topic: library
                .iter()
                .enumerate()
                .find_map(|(index, this_topic)| (*this_topic == self.topic).then_some(index))
                .expect("This should never fail."),
        })
    }
}

/// Equivalent to settings in [DependentEventCount], after their dependencies have been resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ResolvedEventCount {
    /// The topic index to listen to for eventlist messages.
    topic: usize,
}

impl ResolvedMetric for ResolvedEventCount {
    type Dependent = DependentEventCount;
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PartialEventCount {
    count: HashMap<Channel, SumWithSumOfSqrs>,
}

impl PartialMetric for PartialEventCount {
    type Source = ResolvedEventCount;

    fn push(
        &mut self,
        source: &ResolvedMetricShell<Self::Source>,
        channel: Channel,
        _metadata: &FrameMetadata,
        collection_by_topic: &ChannelDataByTopic,
    ) {
        let data = collection_by_topic
            .get(source.resolved.topic)
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
