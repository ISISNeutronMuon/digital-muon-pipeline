use crate::{
    analysis::metrics::{
        MetricOutput, MetricResultError,
        event_counts::CompletedEventCount,
        false_counts::CompletedFalseCount,
        muon_lifetime::CompletedMuonLifetime,
        output::MetricOutputSeries,
        pulse_height_spectra::CompletedPulseHeightSpectra,
        results::{MetricResultByBucket, PartialMetricResultClass},
    },
    engine::PropertyOfMetric,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::error;

pub(crate) trait CompleteMetricResultClass: Clone + Serialize + DeserializeOwned {
    type Partial: PartialMetricResultClass<Complete = Self>;
    type Error: Into<MetricResultError>;
    type Property: Clone;

    fn aggregate(source: &Self::Partial) -> Result<Self, Self::Error>;
    fn get_property(&self, property: Self::Property) -> Result<MetricOutput, Self::Error>;
}

impl<C: CompleteMetricResultClass> MetricResultByBucket<C> {
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

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum CompletedMetricResult {
    EventCount(MetricResultByBucket<CompletedEventCount>),
    FalseCount(MetricResultByBucket<CompletedFalseCount>),
    MuonLifetime(MetricResultByBucket<CompletedMuonLifetime>),
    PulseHeightSpectra(MetricResultByBucket<CompletedPulseHeightSpectra>),
}

impl CompletedMetricResult {
    pub(crate) fn get_aggregate_property(
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
