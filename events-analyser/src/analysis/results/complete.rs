use crate::{
    analysis::results::MetricResultByBucket,
    engine::PropertyOfMetric,
    metric::{
        CompleteMetric, CompleteSeriesOutput, ListOfMetrics, MetricResultError, MetricWrapper,
        complete,
    },
};

impl<C: CompleteMetric> MetricResultByBucket<C> {
    /// Extract a property of the complete metric from the given block index.
    ///
    /// # Parameters
    /// - block: index of the bucket block to extract result from.
    /// - property: the property to extract.
    ///
    /// The caller must ensure the block index is not greater than the number of blocks,
    /// otherwise the method panics.
    pub(super) fn get_property(
        &self,
        block: usize,
        property: C::Property,
    ) -> Result<CompleteSeriesOutput, C::Error>
    where
        Option<CompleteSeriesOutput>: FromIterator<C::Output>,
    {
        let block = self
            .by_bucket
            .get(block)
            .expect("Bucket block should exist, this should never fail.");
        // The result is extracted from each bucket in the block as a sequences of [PartialSeriesOutput]s,
        // the result is concatenated into a [CompleteSeriesOutput].
        let output = block
            .iter()
            .map(|bucket| bucket.get_property(property.clone()))
            .collect::<Result<Option<CompleteSeriesOutput>, _>>()?
            .expect("Buckets should exist, this should never fail.");
        Ok(output)
    }
}

/// Struct to group together all partial metric types.
#[derive(Debug, Clone)]
pub(crate) struct CompleteMetricTypes;

impl ListOfMetrics for CompleteMetricTypes {
    type EventCount = MetricResultByBucket<complete::EventCount>;
    type FalseCount = MetricResultByBucket<complete::FalseCount>;
    type MuonLifetime = MetricResultByBucket<complete::MuonLifetime>;
    type PulseHeightSpectra = MetricResultByBucket<complete::PulseHeightSpectra>;
    type PulsePairTimeDiff = MetricResultByBucket<complete::PulsePairTimeDiff>;
}

/// Stores the complete results of a metric, arranging the results by bucket.
/// Each variant wraps a different concrete instance of [MetricResultByBucket].
///
/// This should only be created by [PartialMetricResult::build_aggregate()], or deserialization.
///
/// [PartialMetricResult] : super::partial
pub(crate) type CompletedMetricResult = MetricWrapper<CompleteMetricTypes>;

impl CompletedMetricResult {
    /// Extract a property of the completed metric from the given block index.
    /// This method assumes the block index is correct and the property is
    /// of the correct type for the metric, and will panic if not.
    ///
    /// # Parameters
    /// - block: index of the bucket block to extract result from.
    /// - property: the property to extract.
    ///
    /// The caller must ensure:
    /// - the block index is not greater than the number of blocks,
    /// - and that the property is of the correct type for the metric,
    ///
    /// otherwise the method panics.
    pub(crate) fn get_property(
        &self,
        block: usize,
        property: PropertyOfMetric,
    ) -> Result<CompleteSeriesOutput, MetricResultError> {
        self.as_ref()
            .zip(property)
            .expect("Property type should match metric type, this should never fail.")
            .fold(
                |(this, that)| this.get_property(block, that),
                |(this, that)| this.get_property(block, that),
                |(this, that)| this.get_property(block, that).map_err(Into::into),
                |(this, that)| this.get_property(block, that),
                |(this, that)| this.get_property(block, that),
            )
    }
}
