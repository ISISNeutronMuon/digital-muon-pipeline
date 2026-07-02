use crate::{
    analysis::{
        BucketIndex,
        metrics::{
            CompleteMetricResultClass, PartialMetricResultClass,
            event_counts::EventCount,
            false_counts::FalseCount,
            muon_lifetime::MuonLifetime,
            results::{MetricResultError, MetricResultStore, complete::CompletedMetricResult},
        },
    },
    engine::{FlatAlgorithm, FlatMetricType, FlatWaveform},
    event::ChannelData, eventlists::ChannelCollection,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

impl<C: PartialMetricResultClass> MetricResultStore<C>
where
    MetricResultError:
        From<<<C as PartialMetricResultClass>::Complete as CompleteMetricResultClass>::Error>,
{
    /// Create new instance from a `Source` instance and a list of the number of buckets in each block.
    /// 
    /// # Parameters
    /// - source: the source of the data, namely the type wrapped by a variant of a [FlatMetricType] instance.
    /// - bucket_block_sizes: the number of buckets in each bucket block.
    pub(super) fn new(source: C::Source, bucket_block_sizes: &[usize]) -> Self {
        let by_bucket = bucket_block_sizes
            .iter()
            .map(|size| vec![C::make_default(&source); *size])
            .collect::<Vec<_>>();
        Self { by_bucket }
    }

    /// Tests whether the amount of data in a specific block exceeds a given value.
    /// 
    /// # Parameters
    /// - block: the block index to test.
    /// - min: the minimum amount of data the block should have.
    pub(crate) fn are_buckets_full_enough(&self, block: usize, min: usize) -> bool {
        self.by_bucket
            .get(block)
            .expect("This should never fail.")
            .iter()
            .all(|c| c.len() >= min)
    }

    /// Adds data to the metric, pushing it to the given bucket index.
    pub(super) fn push(
        &mut self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        bucket_index: BucketIndex,
        collection: &ChannelCollection,
    ) {
        let partial_metric_result = self
            .by_bucket
            .get_mut(bucket_index.block_index)
            .expect("Index should be valid. This should never fail")
            .get_mut(bucket_index.bucket_index)
            .expect("Index should be valid. This should never fail");
        for by_topic in collection.values() {
            partial_metric_result.push(waveform, algorithm, by_topic);
        }
    }

    pub(super) fn aggregate(
        &self,
    ) -> Result<MetricResultStore<C::Complete>, <C::Complete as CompleteMetricResultClass>::Error>
    {
        Ok(MetricResultStore {
            by_bucket: self
                .by_bucket
                .iter()
                .map(|by| {
                    by.iter()
                        .map(C::Complete::aggregate)
                        .collect::<Result<_, _>>()
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

/// Each variant wraps a different concrete instance of [MetricResultStore].
#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum PartialMetricResult {
    /// Descriptive statistics on the count of events.
    EventCount(MetricResultStore<EventCount>),
    /// Descriptive statistics on the count of true/false positive/negative events.
    FalseCount(MetricResultStore<FalseCount>),
    /// Descriptive statistics on the muon-lifetime estimated from the data.
    MuonLifetime(MetricResultStore<MuonLifetime>),
}

impl PartialMetricResult {
    pub(crate) fn new(source: FlatMetricType, bucket_block_sizes: &[usize]) -> Self {
        match source {
            FlatMetricType::EventCount(flat_metric_event_count) => Self::EventCount(
                MetricResultStore::new(flat_metric_event_count, bucket_block_sizes),
            ),
            FlatMetricType::FalseCount(flat_metric_false_count) => Self::FalseCount(
                MetricResultStore::new(flat_metric_false_count, bucket_block_sizes),
            ),
            FlatMetricType::MuonLifetime(flat_metric_muon_lifetime) => Self::MuonLifetime(
                MetricResultStore::new(flat_metric_muon_lifetime, bucket_block_sizes),
            ),
        }
    }

    pub(crate) fn are_buckets_full_enough(&self, block: usize, min: usize) -> bool {
        match self {
            Self::EventCount(patrial_metric_result_class) => {
                patrial_metric_result_class.are_buckets_full_enough(block, min)
            }
            Self::FalseCount(patrial_metric_result_class) => {
                patrial_metric_result_class.are_buckets_full_enough(block, min)
            }
            Self::MuonLifetime(patrial_metric_result_class) => {
                patrial_metric_result_class.are_buckets_full_enough(block, min)
            }
        }
    }

    pub(crate) fn push(
        &mut self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        bucket_index: BucketIndex,
        collection: &ChannelCollection,
    ) {
        match self {
            Self::EventCount(patrial_metric_result_store) => {
                patrial_metric_result_store.push(waveform, algorithm, bucket_index, collection)
            }
            Self::FalseCount(patrial_metric_result_store) => {
                patrial_metric_result_store.push(waveform, algorithm, bucket_index, collection)
            }
            Self::MuonLifetime(patrial_metric_result_store) => {
                patrial_metric_result_store.push(waveform, algorithm, bucket_index, collection)
            }
        }
    }

    pub(crate) fn build_aggregate(&self) -> Result<CompletedMetricResult, MetricResultError> {
        Ok(match self {
            Self::EventCount(patrial_metric_result_store) => {
                CompletedMetricResult::EventCount(patrial_metric_result_store.aggregate()?)
            }
            Self::FalseCount(patrial_metric_result_store) => {
                CompletedMetricResult::FalseCount(patrial_metric_result_store.aggregate()?)
            }
            Self::MuonLifetime(patrial_metric_result_store) => {
                CompletedMetricResult::MuonLifetime(patrial_metric_result_store.aggregate()?)
            }
        })
    }
}
