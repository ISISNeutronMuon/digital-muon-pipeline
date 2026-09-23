use crate::{
    analysis::metrics::{
        MetricOutput, MetricResultError,
        event_counts::CompletedEventCount,
        false_counts::CompletedFalseCount,
        muon_lifetime::CompletedMuonLifetime,
        output::MetricOutputSeries,
        pulse_height_spectra::CompletedPulseHeightSpectra,
        results::{MetricResultByBucket, PartialMetricResultBucket},
    },
    engine::PropertyOfMetric,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::error;

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

impl<C: CompleteMetricResultBucket> MetricResultByBucket<C> {
    pub(super) fn get_property(
        &self,
        block: usize,
        property: C::Property,
    ) -> Result<MetricOutputSeries, C::Error> {
        let block = self
            .by_bucket
            .get(block)
            .expect("Bucket block should exist, this should never fail.");
        let output = block
            .iter()
            .map(|bucket| bucket.get_property(property.clone()))
            .collect::<Result<Option<MetricOutputSeries>, _>>()?
            .expect("Buckets should exist, this should never fail.");
        Ok(output)
    }
}

/// Stores the complete results of a metric, arranging the results by bucket.
/// Each variant wraps a different concrete instance of [MetricResultByBucket].
#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum CompletedMetricResult {
    EventCount(MetricResultByBucket<CompletedEventCount>),
    FalseCount(MetricResultByBucket<CompletedFalseCount>),
    MuonLifetime(MetricResultByBucket<CompletedMuonLifetime>),
    PulseHeightSpectra(MetricResultByBucket<CompletedPulseHeightSpectra>),
}

impl CompletedMetricResult {
    pub(crate) fn get_property(
        &self,
        block: usize,
        property: PropertyOfMetric,
    ) -> Result<MetricOutputSeries, MetricResultError> {
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
            _ => {
                error!("{:?}, {:?}", self, property.clone());
                unreachable!()
            }
        })
    }
}
