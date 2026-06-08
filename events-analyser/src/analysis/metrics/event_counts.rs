use crate::{
    analysis::metrics::{
        CompleteMetricResultClass, MetricOutput, PartialMetricResultClass, SumWithSumOfSqrs,
    },
    engine::{FlatAlgorithm, FlatMetricEventCount, FlatWaveform, MetricProperty},
    event::ChannelData,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct EventCount {
    num: usize,
    topic: usize,
    count: SumWithSumOfSqrs,
}

impl PartialMetricResultClass for EventCount {
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
        collection_by_topic: &[ChannelData],
    ) {
        self.num += 1;
        let data = collection_by_topic
            .get(self.topic)
            .expect("Topic should exist, this should never fail.");
        self.count.add_to(data.get_time_intensity().len() as f64);
    }

    fn len(&self) -> usize {
        self.num
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CompletedEventCount {
    count_mean: f64,
    count_sd: f64,
}

impl CompleteMetricResultClass for CompletedEventCount {
    type Partial = EventCount;

    fn aggregate(source: &Self::Partial) -> Self {
        let (count_mean, count_sd) = source.count.mean_and_stddev(source.num as f64);
        /*    Self::stats_aggregator(source.values(), source.len() as f64, |count| {
            count.count.mean_and_stddev(count.num as f64)
        }); */
        Self {
            count_mean,
            count_sd,
        }
    }

    fn get_property(&self, property: &MetricProperty) -> Result<MetricOutput<f64>, String> {
        match property {
            MetricProperty::Mean => Ok(MetricOutput::Scalar(self.count_mean)),
            MetricProperty::SD => Ok(MetricOutput::ScalarWithBand(self.count_mean, self.count_sd)),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        let true_data = ChannelData::new(vec![(49, 6), (55, 6), (77, 12)]);
        let estimate_data = ChannelData::new(vec![
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
