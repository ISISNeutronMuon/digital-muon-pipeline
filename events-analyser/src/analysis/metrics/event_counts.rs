use std::collections::HashMap;

use crate::{
    analysis::metrics::{
        MetricOutput, MetricResultError,
        results::{CompleteMetricResultBucket, PartialMetricResultBucket},
        utils::{MeanSD, SumWithSumOfSqrs},
    },
    engine::{EventCountProperty, FlatAlgorithm, FlatMetricEventCount, FlatWaveform},
    eventlists::ChannelDataByTopic,
};
use digital_muon_common::Channel;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PartialEventCount {
    num: usize,
    topic: usize,
    count: HashMap<Channel, SumWithSumOfSqrs>,
}

impl PartialMetricResultBucket for PartialEventCount {
    type Source = FlatMetricEventCount;
    type Complete = CompletedEventCount;

    fn make_default(source: &FlatMetricEventCount) -> Self {
        Self {
            num: Default::default(),
            topic: source.topic,
            count: Default::default(),
        }
    }

    fn push(
        &mut self,
        _waveform: &FlatWaveform,
        _algorithm: &FlatAlgorithm,
        channel: Channel,
        collection_by_topic: &ChannelDataByTopic,
    ) {
        self.num += 1;
        let data = collection_by_topic
            .get(self.topic)
            .expect("Topic should exist, this should never fail.");
        self.count
            .entry(channel)
            .or_default()
            .add_to(data.get_time_intensity().len() as f64);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CompletedEventCount {
    count: HashMap<Channel, MeanSD>,
    total_count: MeanSD,
}

impl CompleteMetricResultBucket for CompletedEventCount {
    type Partial = PartialEventCount;
    type Error = MetricResultError;
    type Property = EventCountProperty;

    fn aggregate(source: &Self::Partial) -> Result<Self, MetricResultError> {
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

    fn get_property(&self, property: Self::Property) -> Result<MetricOutput, Self::Error> {
        match property {
            EventCountProperty::TotalMean => Ok(MetricOutput::Value(Some(self.total_count.mean))),
            EventCountProperty::TotalMeanWithSd => Ok(MetricOutput::WithErrors(Some((
                self.total_count.mean,
                self.total_count.sd,
            )))),
            EventCountProperty::ChannelsBoxPlot => Ok(MetricOutput::Group(
                self.count
                    .iter()
                    .map(|(channel, stats)| Some((stats.mean, format!("Channel {channel}"))))
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
