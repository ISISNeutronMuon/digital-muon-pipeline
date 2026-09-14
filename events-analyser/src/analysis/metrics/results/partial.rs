use crate::{
    analysis::{
        BucketIndex,
        metrics::{
            event_counts::PartialEventCount,
            false_counts::PartialFalseCount,
            pulse_height_spectra::PartialPulseHeightSpectra,
            muon_lifetime::PartialMuonLifetime,
            results::{
                CompleteMetricResultClass, MetricObject, MetricResultByBucket, MetricResultError,
                complete::CompletedMetricResult,
            },
        },
    },
    engine::{FlatAlgorithm, FlatBucket, FlatMetricType, FlatWaveform},
    eventlists::{ChannelCollection, ChannelDataByTopic},
};
use digital_muon_common::Channel;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub(crate) trait PartialMetricResultClass: Clone + Serialize + DeserializeOwned {
    type Source;
    type Complete: CompleteMetricResultClass<Partial = Self>;

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

impl<C> MetricObject<C>
where
    C: PartialMetricResultClass,
{
    pub(crate) fn new(source: &C::Source) -> Self {
        Self {
            num_messages: Default::default(),
            object: C::make_default(source),
        }
    }

    pub(crate) fn is_bucket_full_enough(&self, bucket: &FlatBucket) -> bool {
        self.num_messages >= bucket.limits.min
    }

    pub(crate) fn increment_count(&mut self) {
        self.num_messages += 1;
    }

    pub(crate) fn aggregate(
        &self,
    ) -> Result<MetricObject<C::Complete>, <C::Complete as CompleteMetricResultClass>::Error> {
        Ok(MetricObject {
            num_messages: self.num_messages,
            object: C::Complete::aggregate(self)?,
        })
    }
}

impl<C: PartialMetricResultClass> MetricResultByBucket<C>
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
            .map(|size| vec![MetricObject::<C>::new(&source); *size])
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
            .expect("Block index should be valid, this should never fail")
            .get_mut(bucket_index.bucket_index)
            .expect("Bucket index should be valid, this should never fail");

        partial_metric_result.increment_count();
        for (&channel, by_topic) in collection.iter() {
            partial_metric_result
                .object
                .push(waveform, algorithm, channel, by_topic);
        }
    }

    pub(super) fn aggregate(
        &self,
    ) -> Result<MetricResultByBucket<C::Complete>, <C::Complete as CompleteMetricResultClass>::Error>
    {
        Ok(MetricResultByBucket {
            by_bucket: self
                .by_bucket
                .iter()
                .map(|by| {
                    by.iter()
                        .map(MetricObject::aggregate)
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
            FlatMetricType::PulseHeightSpectra(flat_metric_pulse_height_spectra) => Self::PulseHeightSpectra(
                MetricResultByBucket::new(flat_metric_pulse_height_spectra, bucket_block_sizes),
            ),
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
            (Self::PulseHeightSpectra(store), Self::PulseHeightSpectra(source)) => store.load_data(source),
            _ => return Err(MetricResultError::LoadingDataWrongMetrics),
        }
        Ok(())
    }
}
