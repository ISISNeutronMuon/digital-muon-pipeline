use crate::{
    analysis::metrics::{
        CompleteMetricResultClass, MetricOutput, event_counts::CompletedEventCount, false_counts::CompletedFalseCount, intensity_graph::CompletedIntensityGraph, muon_lifetime::CompletedMuonLifetime, results::MetricResultStore
    },
    engine::MetricProperty,
};
use serde::{Deserialize, Serialize};

impl<C: CompleteMetricResultClass> MetricResultStore<C> {
    pub(super) fn get_property(
        &self,
        block: usize,
        property: &MetricProperty,
    ) -> Result<MetricOutput<Vec<f64>>, String> {
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
        }
        .ok_or_else(|| "No buckets, this should never fail.".to_string())
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) enum CompletedMetricResult {
    EventCount(MetricResultStore<CompletedEventCount>),
    FalseCount(MetricResultStore<CompletedFalseCount>),
    MuonLifetime(MetricResultStore<CompletedMuonLifetime>),
    IntensityGraph(MetricResultStore<CompletedIntensityGraph>),
}

impl CompletedMetricResult {
    pub(crate) fn get_aggregate_property(
        &self,
        block: usize,
        property: &MetricProperty,
    ) -> Result<MetricOutput<Vec<f64>>, String> {
        match self {
            Self::EventCount(completed) => completed.get_property(block, property),
            Self::FalseCount(completed) => completed.get_property(block, property),
            Self::MuonLifetime(completed) => completed.get_property(block, property),
            Self::IntensityGraph(completed) => completed.get_property(block, property),
        }
    }
}
