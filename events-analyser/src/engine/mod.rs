mod elements;
mod utils;
mod values;

use serde::Deserialize;
use std::ops::Deref;
use crate::engine::{
    elements::{
        Algorithm, BucketBlock, BucketBlockTemplate, BucketError, ChartError, Criteria, Waveform,
    },
    utils::WithSource,
    values::ValueError,
};

pub(crate) use crate::engine::{
    elements::{
        Chart, FlatAlgorithm, FlatBucketBlock, FlatChart, FlatMetric, FlatMetricFalseCount,
        FlatWaveform, Metric, FlatSeries
    },
    utils::WithName,
};

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Templates {
    // List of metrics to calculate for each phase.
    pub(crate) criteria_templates: Vec<WithName<Criteria>>,
    // List of metrics to calculate for each phase.
    pub(crate) arrays: Vec<WithName<Array>>,
    // List of metrics to calculate for each phase.
    pub(crate) algorithms: Vec<WithName<Algorithm>>,
    // List of metrics to calculate for each phase.
    pub(crate) bucket_templates: Vec<WithName<BucketBlockTemplate>>,
    // List of metrics to calculate for each phase.
    pub(crate) waveforms: Vec<WithName<Waveform>>,
}

impl Templates {
    fn get_bucket(
        &self,
        object: &WithSource<WithName<BucketBlock>>,
    ) -> Option<&BucketBlockTemplate> {
        self.bucket_templates
            .iter()
            .find_map(|tmplt| tmplt.is_source(object).then_some(tmplt.deref()))
    }

    fn get_arrays(&self) -> &[WithName<Array>] {
        &self.arrays
    }

    fn get_criteria(&self, name: &str) -> Option<&Criteria> {
        self.criteria_templates
            .iter()
            .find_map(|tmplt| tmplt.has_name(name).then_some(tmplt.deref()))
    }

    fn get_algorithm(&self, name: &str) -> Option<&Algorithm> {
        self.algorithms
            .iter()
            .find_map(|alg| alg.has_name(name).then_some(alg.deref()))
    }

    fn get_waveform(&self, name: &str) -> Option<&Waveform> {
        self.waveforms
            .iter()
            .find_map(|wav| wav.has_name(name).then_some(wav.deref()))
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct AnalysisSettings {
    // Topics the consumer should listen to to receive Eventlist messages.
    pub(crate) events_topics: Vec<String>,
    // List of metrics to calculate and are available to the charts.
    pub(crate) metrics: Vec<WithName<Metric>>,
    // Templates of structures that are used when metrics, buckets, and charts are flattened.
    pub(crate) templates: Templates,
    // Blocks of buckets that accept collections of eventlists.
    pub(crate) buckets: Vec<WithSource<WithName<BucketBlock>>>,
    // List of Charts.
    pub(crate) charts: Vec<Chart>,
}

/// Provides methods for flattening dependencies.
pub(crate) trait Flattenable<Lib> {
    /// Resulting Type with dependencies flattened.
    type Flat;
    /// Error type.
    type Error;

    /// Embeds any dependencies of the type.
    ///
    /// # Parameters
    /// - library: dependencies referenced by the type are passed in here.
    fn flatten(&self, library: Lib) -> Result<Self::Flat, Self::Error>;
}

/// Provides methods for flattening dependencies with additional index parameter.
trait FlattenableWithIndex {
    /// Resulting type upon flattening.
    type Flat;
    /// Structure that can be referenced during flattening.
    type Library: ?Sized;
    /// Error type.
    type Error;

    /// Embeds any dependencies of the type.
    ///
    /// # Parameters
    /// - library: dependencies referenced by the type are passed in here.
    /// - index: FIXME.
    fn flatten(&self, library: &Self::Library, index: usize) -> Result<Self::Flat, Self::Error>;
}

impl AnalysisSettings {
    pub(crate) fn flatten_buckets(&self) -> Result<Vec<WithName<FlatBucketBlock>>, BucketError> {
        self.buckets
            .iter()
            .map(|block| block.flatten(&self.templates))
            .collect::<Result<_, BucketError>>()
    }

    pub(crate) fn flatten_metrics(&self) -> Result<Vec<FlatMetric>, ValueError> {
        self.metrics
            .iter()
            .map(|metric| metric.flatten(&self.events_topics))
            .collect::<Result<_, ValueError>>()
    }

    pub(crate) fn flatten_charts(
        &self,
        buckets: &[WithName<FlatBucketBlock>],
    ) -> Result<Vec<FlatChart>, ChartError> {
        self.charts
            .iter()
            .map(|chart| chart.flatten((self, buckets)))
            .collect::<Result<_, ChartError>>()
    }

    pub(crate) fn get_bucket_block_index(&self, name: &str) -> Option<usize> {
        self.buckets
            .iter()
            .enumerate()
            .find_map(|(index, block)| (block.has_name(name)).then_some(index))
    }

    pub(crate) fn get_metric_index(&self, name: &str) -> Option<usize> {
        self.metrics
            .iter()
            .enumerate()
            .find_map(|(index, metric)| metric.has_name(name).then_some(index))
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Array {
    // Is applied to all voltages when traces are created
    pub(crate) values: Vec<f64>,
}

impl Array {
    pub(crate) fn get_element(&self, index: usize) -> f64 {
        *self.values.get(index).unwrap() // FIXME: Handle Error
    }
}

#[cfg(test)]
mod tests {
    //use super::*;

    //const JSON_INPUT_1: &str = r#""#;
    #[test]
    fn test1() {
        //let simulation: AnalysisSettings = serde_json::from_str(JSON_INPUT_1).unwrap();
    }
}
