use crate::{
    analysis::metrics::{
        results::MetricResultByBucket,
    },
    engine::PropertyOfMetric, metric::{CompleteMetric, CompleteSeriesOutput, complete},
    metric::MetricResultError
};
use serde::{Deserialize, Serialize};
use tracing::error;
/*
/// Encapsulates the completed results of a metric, aggregated from instances
/// of objects implementing `PartialMetricResultBucket`.
pub(crate) trait CompleteMetricResultBucket: Clone + Serialize + DeserializeOwned {
    /// The corresponding partial results type.
    type Partial: PartialMetricResultBucket<Complete = Self>;
    /// Error type.
    type Error: Into<MetricResultError>;
    /// Type which specifies a particular property of the metric.
    type Property: Clone;

    /// Creates an instance of this object by aggregating the results from a partial results object.
    fn aggregate(source: &Self::Partial) -> Result<Self, Self::Error>;
    /// Extract a particular property of the results.
    fn get_property(&self, property: Self::Property) -> Result<MetricOutput, Self::Error>;
}
 */
impl<C: CompleteMetric> MetricResultByBucket<C> {
    pub(super) fn get_property(
        &self,
        block: usize,
        property: C::Property,
    ) -> Result<CompleteSeriesOutput, C::Error>
    where Option<CompleteSeriesOutput>: FromIterator<C::Output> {
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

/// Stores the complete results of a metric, arranging the results by bucket.
/// Each variant wraps a different concrete instance of [MetricResultByBucket].
#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum CompletedMetricResult {
    EventCount(MetricResultByBucket<complete::EventCount>),
    FalseCount(MetricResultByBucket<complete::FalseCount>),
    MuonLifetime(MetricResultByBucket<complete::MuonLifetime>),
    PulseHeightSpectra(MetricResultByBucket<complete::PulseHeightSpectra>),
    PulsePairDifferences(MetricResultByBucket<complete::PulsePairTimeDiff>),
}

impl CompletedMetricResult {
    pub(crate) fn get_property(
        &self,
        block: usize,
        property: PropertyOfMetric,
    ) -> Result<CompleteSeriesOutput, MetricResultError> {
        Ok(match (self, property.clone()) {
            (Self::EventCount(completed), PropertyOfMetric::EventCount(property)) => {
                completed.get_property(block, property)?
            }
            (Self::FalseCount(completed), PropertyOfMetric::FalseCount(property)) => {
                completed.get_property(block, property)?
            }
            (Self::MuonLifetime(completed), PropertyOfMetric::MuonLifetime(property)) => {
                completed.get_property(block, property)?
            }
            (
                Self::PulseHeightSpectra(completed),
                PropertyOfMetric::PulseHeightSpectra(property),
            ) => completed.get_property(block, property)?,
            (
                Self::PulsePairDifferences(completed),
                PropertyOfMetric::PulsePairDifferences(property),
            ) => completed.get_property(block, property)?,
            _ => {
                error!("{:?}, {:?}", self, property.clone());
                unreachable!()
            }
        })
    }
}
