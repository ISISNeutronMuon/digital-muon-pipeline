use crate::{
    analysis::{
        BucketIndex,
        results::{
            MetricResultBucketWrapper, MetricResultByBucket, complete::CompletedMetricResult,
        },
    },
    engine::{FlatBucket, FlatMetricType},
    eventlists::ChannelCollection,
    metric::{
        CompleteMetric, FlatMetricShell, MetricResultError, MetricWrapper, PartialMetric,
        PartialMetricShell, partial,
    },
};

impl<C: PartialMetric> MetricResultBucketWrapper<PartialMetricShell<C>> {
    /// Create a new and empty partial results storage object.
    pub(crate) fn new(source: &FlatMetricShell<<C as PartialMetric>::Source>) -> Self {
        Self {
            num_messages: Default::default(),
            object: PartialMetricShell::make_default(source),
        }
    }

    /// Tests whether the bucket has enough data to be aggregrated.
    pub(crate) fn is_bucket_full_enough(&self, bucket: &FlatBucket) -> bool {
        self.num_messages >= bucket.limits.get_min()
    }

    /// Increate message count.
    pub(crate) fn increment_count(&mut self) {
        self.num_messages += 1;
    }

    /// Create aggregated version of this type.
    pub(crate) fn aggregate<C2: CompleteMetric<Partial = C>>(
        &self,
    ) -> Result<MetricResultBucketWrapper<C2>, C2::Error> {
        Ok(MetricResultBucketWrapper {
            num_messages: self.num_messages,
            object: C2::aggregate(self)?,
        })
    }
}

impl<P> MetricResultByBucket<PartialMetricShell<P>>
where
    P: PartialMetric,
{
    /// Create new instance from a `Source` instance and a list of the number of buckets in each block.
    ///
    /// # Parameters
    /// - source: the source of the data, namely the type wrapped by a variant of a [FlatMetricType] instance.
    /// - bucket_block_sizes: the number of buckets in each bucket block.
    pub(super) fn new(source: FlatMetricShell<P::Source>, bucket_block_sizes: &[usize]) -> Self {
        let by_bucket = bucket_block_sizes
            .iter()
            .map(|size| {
                vec![MetricResultBucketWrapper::<PartialMetricShell<P>>::new(&source); *size]
            })
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
    fn get_bucket_mut(
        &mut self,
        bucket_index: BucketIndex,
    ) -> &mut MetricResultBucketWrapper<PartialMetricShell<P>> {
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
    pub(super) fn push(&mut self, bucket_index: BucketIndex, collection: &ChannelCollection) {
        let partial_metric_result = self.get_bucket_mut(bucket_index);
        partial_metric_result.increment_count();
        for (&channel, by_topic) in collection.iter() {
            partial_metric_result.object.push(channel, by_topic);
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

    pub(super) fn aggregate<C: CompleteMetric<Partial = P>>(
        &self,
    ) -> Result<MetricResultByBucket<C>, C::Error> {
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
pub(crate) type PartialMetricResult = MetricWrapper<
    MetricResultByBucket<PartialMetricShell<partial::EventCount>>,
    MetricResultByBucket<PartialMetricShell<partial::FalseCount>>,
    MetricResultByBucket<PartialMetricShell<partial::MuonLifetime>>,
    MetricResultByBucket<PartialMetricShell<partial::PulseHeightSpectra>>,
    MetricResultByBucket<PartialMetricShell<partial::PulsePairTimeDiff>>,
>;

impl PartialMetricResult {
    pub(crate) fn new(source: FlatMetricType, bucket_block_sizes: &[usize]) -> Self {
        source.map(
            |this| MetricResultByBucket::new(this, bucket_block_sizes),
            |this| MetricResultByBucket::new(this, bucket_block_sizes),
            |this| MetricResultByBucket::new(this, bucket_block_sizes),
            |this| MetricResultByBucket::new(this, bucket_block_sizes),
            |this| MetricResultByBucket::new(this, bucket_block_sizes),
        )
    }

    pub(crate) fn are_buckets_full_enough(&self, block: usize, min: &[FlatBucket]) -> bool {
        self.as_ref().fold(
            |this| this.are_buckets_full_enough(block, min),
            |this| this.are_buckets_full_enough(block, min),
            |this| this.are_buckets_full_enough(block, min),
            |this| this.are_buckets_full_enough(block, min),
            |this| this.are_buckets_full_enough(block, min),
        )
    }

    pub(crate) fn push(&mut self, bucket_index: BucketIndex, collection: &ChannelCollection) {
        self.as_mut().fold(
            |this| this.push(bucket_index, collection),
            |this| this.push(bucket_index, collection),
            |this| this.push(bucket_index, collection),
            |this| this.push(bucket_index, collection),
            |this| this.push(bucket_index, collection),
        )
    }

    pub(crate) fn build_aggregate(&self) -> Result<CompletedMetricResult, MetricResultError> {
        self.as_ref().try_map(
            MetricResultByBucket::aggregate,
            MetricResultByBucket::aggregate,
            MetricResultByBucket::aggregate,
            MetricResultByBucket::aggregate,
            MetricResultByBucket::aggregate,
        )
    }

    pub(crate) fn load_data(&mut self, source: &Self) -> Result<(), MetricResultError> {
        self.as_mut().zip(source.as_ref()).fold(
            |(metric_result_by_bucket, source)| metric_result_by_bucket.load_data(source),
            |(metric_result_by_bucket, source)| metric_result_by_bucket.load_data(source),
            |(metric_result_by_bucket, source)| metric_result_by_bucket.load_data(source),
            |(metric_result_by_bucket, source)| metric_result_by_bucket.load_data(source),
            |(metric_result_by_bucket, source)| metric_result_by_bucket.load_data(source),
        );
        Ok(())
    }
}
