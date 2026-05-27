mod utils;
mod values;
mod elements;
use serde::Deserialize;

use crate::engine::{
    elements::{
        Algorithm, BucketBlock, Criteria, Waveform
    },
    utils::NameValueTemplate,
};

pub(crate) use crate::engine::elements::{
    Chart,
    FlatBucket,
    FlatBucketBlock,
    FlatChart,
    FlatMetric,
    FlatMetricFalseCount,
    Metric,
    FlatAlgorithm,
    FlatWaveform
};

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Templates {
    // List of metrics to calculate for each phase.
    pub(crate) criteria_templates: Vec<Criteria>,
    // List of metrics to calculate for each phase.
    pub(crate) arrays: Vec<Array>,
    // List of metrics to calculate for each phase.
    pub(crate) algorithms: Vec<NameValueTemplate<Algorithm>>,
    // List of metrics to calculate for each phase.
    pub(crate) waveforms: Vec<NameValueTemplate<Waveform>>,
}

impl Templates {
    fn get_criteria(&self) -> &[Criteria] {
        &self.criteria_templates
    }

    fn get_arrays(&self) -> &[Array] {
        &self.arrays
    }

    fn get_algorithm(&self, source: &str) -> Option<&Algorithm> {
        self.algorithms
            .iter()
            .find_map(|alg| alg.has_name(source).then_some(alg.get_value()))
    }

    fn get_waveform(&self, source: &str) -> Option<&Waveform> {
        self.waveforms
            .iter()
            .find_map(|wav| wav.has_name(source).then_some(wav.get_value()))
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct AnalysisSettings {
    // These topics are interpreted as the results of detector
    pub(crate) events_topics: Vec<String>,
    // List of metrics to calculate for each phase.
    pub(crate) metrics: Vec<Metric>,
    // List of metrics to calculate for each phase.
    pub(crate) templates: Templates,
    // List of phases in the analysis.
    pub(crate) buckets: Vec<BucketBlock>,
    // List of phases in the analysis.
    pub(crate) charts: Vec<Chart>,
}

pub(crate) trait Flattenable {
    type Flat;
    type Library: ?Sized;
    type Error;

    fn flatten(&self, library: &Self::Library) -> Result<Self::Flat, Self::Error>;
}

trait FlattenableWithIndex {
    type Flat;
    type Library: ?Sized;
    type Error;

    fn flatten(&self, library: &Self::Library, index: usize) -> Result<Self::Flat, Self::Error>;
}

impl AnalysisSettings {
    pub(crate) fn flatten_buckets(&self) -> Result<Vec<FlatBucketBlock>, String> {
        self.buckets
            .iter()
            .map(|block| block.flatten(&self.templates))
            .collect::<Result<_, String>>()
    }
    
    pub(crate) fn flatten_metrics(&self) -> Result<Vec<FlatMetric>, String> {
        self.metrics
            .iter()
            .map(|metric| metric.flatten(&self.events_topics))
            .collect::<Result<_, String>>()
    }

    pub(crate) fn get_bucket_block_index_if<F>(&self, name: &str, when: F) -> Option<usize> where F : Fn(&BucketBlock) -> bool {
        self.buckets.iter()
            .enumerate()
            .filter(|(_,x)|when(x))
            .find_map(|(index, block)|
                (block.name == name).then_some(index)
            )
    }

    pub(crate) fn get_metric_index(&self, name: &str) -> Option<usize> {
        self.metrics.iter()
            .enumerate()
            .find_map(|(index, metric)|
                metric.has_name(name).then_some(index)
            )
    }
    
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Array {
    // Is applied to all voltages when traces are created
    pub(crate) name: String,
    // Is applied to all voltages when traces are created
    pub(crate) values: Vec<f64>,
}

impl Array {
    pub(crate) fn has_name(&self, name: &str) -> bool {
        self.name == name
    }

    pub(crate) fn get_element(&self, index: usize) -> f64 {
        self.values.get(index).unwrap().clone() // FIXME: Handle Error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON_INPUT_1: &str = r#""#;
    #[test]
    fn test1() {
        let simulation: AnalysisSettings = serde_json::from_str(JSON_INPUT_1).unwrap();
    }
}
