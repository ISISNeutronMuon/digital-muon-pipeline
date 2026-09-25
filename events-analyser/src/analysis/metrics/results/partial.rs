use crate::{
    analysis::{
        BucketIndex,
        metrics::{
            event_counts::PartialEventCount,
            false_counts::PartialFalseCount,
            muon_lifetime::PartialMuonLifetime,
            pulse_height_spectra::PartialPulseHeightSpectra,
            results::{
                CompleteMetricResultBucket, MetricResultBucketWrapper, MetricResultByBucket,
                MetricResultError, complete::CompletedMetricResult,
            },
        },
    },
    engine::{FlatAlgorithm, FlatBucket, FlatMetricType, FlatWaveform},
    eventlists::{ChannelCollection, ChannelDataByTopic},
};
use digital_muon_common::Channel;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub(crate) trait PartialMetricResultBucket: Clone + Serialize + DeserializeOwned {
    type Source;
    type Complete: CompleteMetricResultBucket<Partial = Self>;

    fn make_default(source: &Self::Source) -> Self;
    fn load_data(&mut self, source: &Self) {
        *self = source.clone();
    }
    fn push(
        &mut self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        channel: Channel,
        by_topic: &ChannelDataByTopic,
    );
}

impl<C> MetricResultBucketWrapper<C>
where
    C: PartialMetricResultBucket,
{
    /// Create a new and empty partial results storage object.
    pub(crate) fn new(source: &C::Source) -> Self {
        Self {
            num_messages: Default::default(),
            object: C::make_default(source),
        }
    }

    /// Tests whether the bucket has enough data to be aggregrated.
    pub(crate) fn is_bucket_full_enough(&self, bucket: &FlatBucket) -> bool {
        self.num_messages >= bucket.limits.min
    }

    /// Increate message count.
    pub(crate) fn increment_count(&mut self) {
        self.num_messages += 1;
    }

    /// Create aggregated version of this type.
    pub(crate) fn aggregate(
        &self,
    ) -> Result<
        MetricResultBucketWrapper<C::Complete>,
        <C::Complete as CompleteMetricResultBucket>::Error,
    > {
        Ok(MetricResultBucketWrapper {
            num_messages: self.num_messages,
            object: C::Complete::aggregate(self)?,
        })
    }
}

impl<C> MetricResultByBucket<C>
where
    C: PartialMetricResultBucket,
    MetricResultError:
        From<<<C as PartialMetricResultBucket>::Complete as CompleteMetricResultBucket>::Error>,
{
    /// Create new instance from a `Source` instance and a list of the number of buckets in each block.
    ///
    /// # Parameters
    /// - source: the source of the data, namely the type wrapped by a variant of a [FlatMetricType] instance.
    /// - bucket_block_sizes: the number of buckets in each bucket block.
    pub(super) fn new(source: C::Source, bucket_block_sizes: &[usize]) -> Self {
        let by_bucket = bucket_block_sizes
            .iter()
            .map(|size| vec![MetricResultBucketWrapper::<C>::new(&source); *size])
            .collect::<Vec<_>>();
        Self { by_bucket }
    }

    /// Tests whether the amount of data in a specific block exceeds a given value.
    ///
    /// # Parameters
    /// - block: the block index to test.
    /// - min: the minimum amount of data the block should have.
    pub(crate) fn are_buckets_full_enough(&self, block: usize, buckets: &[FlatBucket]) -> bool {
        self.by_bucket
            .get(block)
            .expect("This should never fail.")
            .iter()
            .zip(buckets.iter())
            .all(|(store_object, bucket)| store_object.is_bucket_full_enough(bucket))
    }

    /// Obtain a mutable reference to the bucket specified by the given index.
    ///
    /// # Parameters
    /// - bucket_index: the bucket to obtain.
    fn get_bucket_mut(&mut self, bucket_index: BucketIndex) -> &mut MetricResultBucketWrapper<C> {
        self.by_bucket
            .get_mut(bucket_index.block_index)
            .expect("Block index should be valid, this should never fail")
            .get_mut(bucket_index.bucket_index)
            .expect("Bucket index should be valid, this should never fail")
    }

    /// Adds data to the metric, pushing it to the given bucket index.
    ///
    /// # Parameters
    /// - bucket_index: the bucket to push to.
    pub(super) fn push(
        &mut self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        bucket_index: BucketIndex,
        collection: &ChannelCollection,
    ) {
        let partial_metric_result = self.get_bucket_mut(bucket_index);
        partial_metric_result.increment_count();
        for (&channel, by_topic) in collection.iter() {
            partial_metric_result
                .object
                .push(waveform, algorithm, channel, by_topic);
        }
    }

    /// Loads the results from an external source.
    pub(crate) fn load_data(&mut self, source: &Self) {
        for (bucket, source_bucket) in Iterator::zip(
            self.by_bucket.iter_mut().flatten(),
            source.by_bucket.iter().flatten(),
        ) {
            bucket.num_messages = source_bucket.num_messages;
            bucket.object.load_data(&source_bucket.object);
        }
    }

    pub(super) fn aggregate(
        &self,
    ) -> Result<MetricResultByBucket<C::Complete>, <C::Complete as CompleteMetricResultBucket>::Error>
    {
        Ok(MetricResultByBucket {
            by_bucket: self
                .by_bucket
                .iter()
                .map(|by| {
                    by.iter()
                        .map(MetricResultBucketWrapper::aggregate)
                        .collect::<Result<_, _>>()
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

/// Stores the partial results of a metric, arranging the results by bucket.
/// Each variant wraps a different concrete instance of [MetricResultByBucket].
#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum PartialMetricResult {
    /// Descriptive statistics on the count of events.
    EventCount(MetricResultByBucket<PartialEventCount>),
    /// Descriptive statistics on the count of true/false positive/negative events.
    FalseCount(MetricResultByBucket<PartialFalseCount>),
    /// Descriptive statistics on the muon-lifetime estimated from the data.
    MuonLifetime(MetricResultByBucket<PartialMuonLifetime>),
    /// Descriptive statistics on the muon-lifetime estimated from the data.
    PulseHeightSpectra(MetricResultByBucket<PartialPulseHeightSpectra>),
}

impl PartialMetricResult {
    pub(crate) fn new(source: FlatMetricType, bucket_block_sizes: &[usize]) -> Self {
        match source {
            FlatMetricType::EventCount(flat_metric_event_count) => Self::EventCount(
                MetricResultByBucket::new(flat_metric_event_count, bucket_block_sizes),
            ),
            FlatMetricType::FalseCount(flat_metric_false_count) => Self::FalseCount(
                MetricResultByBucket::new(flat_metric_false_count, bucket_block_sizes),
            ),
            FlatMetricType::MuonLifetime(flat_metric_muon_lifetime) => Self::MuonLifetime(
                MetricResultByBucket::new(flat_metric_muon_lifetime, bucket_block_sizes),
            ),
            FlatMetricType::PulseHeightSpectra(flat_metric_pulse_height_spectra) => {
                Self::PulseHeightSpectra(MetricResultByBucket::new(
                    flat_metric_pulse_height_spectra,
                    bucket_block_sizes,
                ))
            }
        }
    }

    pub(crate) fn are_buckets_full_enough(&self, block: usize, min: &[FlatBucket]) -> bool {
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
            Self::PulseHeightSpectra(patrial_metric_result_class) => {
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
            Self::PulseHeightSpectra(patrial_metric_result_store) => {
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
            Self::PulseHeightSpectra(patrial_metric_result_store) => {
                CompletedMetricResult::PulseHeightSpectra(patrial_metric_result_store.aggregate()?)
            }
        })
    }

    pub(crate) fn load_data(&mut self, source: &Self) -> Result<(), MetricResultError> {
        match (self, source) {
            (Self::EventCount(store), Self::EventCount(source)) => store.load_data(source),
            (Self::FalseCount(store), Self::FalseCount(source)) => store.load_data(source),
            (Self::MuonLifetime(store), Self::MuonLifetime(source)) => store.load_data(source),
            (Self::PulseHeightSpectra(store), Self::PulseHeightSpectra(source)) => {
                store.load_data(source)
            }
            _ => return Err(MetricResultError::LoadingDataWrongMetrics),
        }
        Ok(())
    }
}
