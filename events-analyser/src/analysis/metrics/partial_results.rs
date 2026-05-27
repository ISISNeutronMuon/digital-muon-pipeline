use std::collections::HashMap;

use digital_muon_common::Channel;

use crate::{
    analysis::metrics::{MetricAggregatedResult, MetricChannelResult, false_counts::FalseCount},
    engine::{FlatAlgorithm, FlatMetric, FlatWaveform},
    event::ChannelData,
};
#[derive(Clone)]
pub(crate) struct PatrialMetricResultClass<C>
where
    C: MetricChannelResult,
{
    default: C,
    by_channel_and_bucket: Vec<Vec<HashMap<Channel, C>>>,
}

impl<C> PatrialMetricResultClass<C>
where
    C: MetricChannelResult,
{
    pub(crate) fn new(source: C::Source, bucket_block_sizes: &[usize]) -> Self {
        let default = C::make_default(source);
        let by_channel_and_bucket = bucket_block_sizes
            .iter()
            .map(|size| vec![Default::default(); *size])
            .collect::<Vec<_>>();
        Self {
            default,
            by_channel_and_bucket,
        }
    }

    pub(crate) fn push(
        &mut self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        bucket_index: (usize, usize),
        collection: &HashMap<u32, Vec<ChannelData>>,
    ) {
        let by_channel = self
            .by_channel_and_bucket
            .get_mut(bucket_index.0)
            .expect("Index should be valid. This should never fail")
            .get_mut(bucket_index.1)
            .expect("Index should be valid. This should never fail");

        for (&channel, by_topic) in collection {
            let result_params = by_channel.entry(channel).or_insert(self.default.clone());
            result_params.push(waveform, algorithm, by_topic);
        }
    }

    pub(crate) fn aggregate(&self) -> Vec<Vec<C::Aggregrate>> {
        self.by_channel_and_bucket
            .iter()
            .map(|by| {
                by.iter()
                    .map(C::Aggregrate::aggregate)
                    .collect()
            })
            .collect()
    }
}

#[derive(Clone)]
pub(crate) enum PatrtialMetricResult {
    FalseCount(PatrialMetricResultClass<FalseCount>),
}

impl PatrtialMetricResult {
    pub(crate) fn new(source: FlatMetric, bucket_block_sizes: &[usize]) -> Self {
        match source {
            FlatMetric::FalseCount(flat_metric_false_count) => Self::FalseCount(
                PatrialMetricResultClass::new(flat_metric_false_count, bucket_block_sizes),
            ),
        }
    }

    pub(crate) fn push(
        &mut self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        bucket_index: (usize, usize),
        collection: &HashMap<u32, Vec<ChannelData>>,
    ) {
        match self {
            PatrtialMetricResult::FalseCount(patrial_metric_result_class) => {
                patrial_metric_result_class.push(waveform, algorithm, bucket_index, collection)
            }
        }
    }

    pub(crate) fn aggregate(&self) {
        match self {
            PatrtialMetricResult::FalseCount(patrial_metric_result_class) => {
                patrial_metric_result_class.aggregate()
            }
        };
    }
}
