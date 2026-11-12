use crate::{
    analysis::results::MetricResultByBucket,
    engine::PropertyOfMetric,
    metric::{
        CompleteMetric, CompleteSeriesOutput, ListOfMetrics, MetricResultError, MetricWrapper,
        complete,
    },
};

impl<C: CompleteMetric> MetricResultByBucket<C> {
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
        let output = block
            .iter()
            .map(|bucket| bucket.get_property(property.clone()))
            .collect::<Result<Option<CompleteSeriesOutput>, _>>()?
            .expect("Buckets should exist, this should never fail.");
        Ok(output)
    }
}

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
pub(crate) type CompletedMetricResult = MetricWrapper<CompleteMetricTypes>;

impl CompletedMetricResult {
    pub(crate) fn get_property(
        &self,
        block: usize,
        property: PropertyOfMetric,
    ) -> Result<CompleteSeriesOutput, MetricResultError> {
        self.as_ref().zip(property).fold(
            |(this, that)| Ok(this.get_property(block, that)?),
            |(this, that)| Ok(this.get_property(block, that)?),
            |(this, that)| Ok(this.get_property(block, that)?),
            |(this, that)| Ok(this.get_property(block, that)?),
            |(this, that)| Ok(this.get_property(block, that)?),
        )
    }
}
