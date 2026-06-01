use std::collections::HashMap;

use digital_muon_common::Channel;

use crate::{
    analysis::metrics::{
        MetricAggregatedResult, MetricChannelResult, MetricOutput, false_counts::FalseCount,
        muon_lifetime::MuonLifetime,
    },
    engine::{FlatAlgorithm, FlatMetric, FlatWaveform},
    event::ChannelData,
};

#[derive(Clone)]
pub(crate) struct PatrtialMetricResultClass<C>
where
    C: MetricChannelResult,
{
    default: C,
    by_channel_and_bucket: Vec<Vec<HashMap<Channel, C>>>,
}

#[derive(Clone)]
pub(crate) struct CompletedMetricResultClass<C>
where
    C: MetricChannelResult,
{
    by_bucket: Vec<Vec<C::Aggregrate>>,
}

impl<C: MetricChannelResult> CompletedMetricResultClass<C> {
    fn get_property(&self, block: usize, property: &str) -> Result<MetricOutput<Vec<f64>>, String> {
        let block = self.by_bucket.get(block).expect("This should never fail.");
        if let Some((first, rest)) = block.split_first() {
            let mut agg: MetricOutput<Vec<f64>> = first
                .get_property(property)?
                .to_vector(self.by_bucket.len());
            
            for metric in rest {
                agg.append(&metric.get_property(property)?);
            }
            Some(agg)
        } else {
            None
        }.ok_or_else(|| format!("No buckets, this should never fail."))
    }
}

impl<C> PatrtialMetricResultClass<C>
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
            .map(|by| by.iter().map(C::Aggregrate::aggregate).collect())
            .collect()
    }
}

#[derive(Clone)]
pub(crate) enum PatrtialMetricResult {
    FalseCount(
        PatrtialMetricResultClass<FalseCount>,
        Option<CompletedMetricResultClass<FalseCount>>,
    ),
    MuonLifetime(
        PatrtialMetricResultClass<MuonLifetime>,
        Option<CompletedMetricResultClass<MuonLifetime>>,
    ),
}

impl PatrtialMetricResult {
    pub(crate) fn new(source: FlatMetric, bucket_block_sizes: &[usize]) -> Self {
        match source {
            FlatMetric::FalseCount(flat_metric_false_count) => Self::FalseCount(
                PatrtialMetricResultClass::new(flat_metric_false_count, bucket_block_sizes),
                None,
            ),
            FlatMetric::MuonLifetime => {
                Self::MuonLifetime(PatrtialMetricResultClass::new((), bucket_block_sizes), None)
            }
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
            PatrtialMetricResult::FalseCount(patrial_metric_result_class, _) => {
                patrial_metric_result_class.push(waveform, algorithm, bucket_index, collection)
            }
            PatrtialMetricResult::MuonLifetime(patrial_metric_result_class, _) => {
                patrial_metric_result_class.push(waveform, algorithm, bucket_index, collection)
            }
        }
    }

    pub(crate) fn build_aggregate(&mut self) {
        match self {
            PatrtialMetricResult::FalseCount(patrial_metric_result_class, completed) => {
                if completed.is_none() {
                    completed.replace(CompletedMetricResultClass {
                        by_bucket: patrial_metric_result_class.aggregate(),
                    });
                }
            }
            PatrtialMetricResult::MuonLifetime(patrial_metric_result_class, completed) => {
                if completed.is_none() {
                    completed.replace(CompletedMetricResultClass {
                        by_bucket: patrial_metric_result_class.aggregate(),
                    });
                }
            }
        };
    }

    pub(crate) fn get_aggregate_property(
        &self,
        block: usize,
        property: &str,
    ) -> Result<MetricOutput<Vec<f64>>, String> {
        match self {
            PatrtialMetricResult::FalseCount(_, Some(completed)) => {
                completed.get_property(block, property)
            }
            PatrtialMetricResult::FalseCount(_, None) => Err("False Count Not Aggregated".into()),
            PatrtialMetricResult::MuonLifetime(_, Some(completed)) => {
                completed.get_property(block, property)
            }
            PatrtialMetricResult::MuonLifetime(_, None) => {
                Err("Muon Lifetime Not Aggregated".into())
            }
        }
    }
}
