use crate::engine::{
    Chart, FlatBucketBlock, FlatChart, FlatMetric, FlatPage, Flattenable, HasName, Metric, Page,
    PageError,
    elements::{
        Algorithm, AlgorithmProperties, BucketBlock, BucketBlockProperties, BucketBlockTemplate,
        BucketError, ChartError, CriteriaTemplate, Waveform, WaveformProperties,
    },
    values::ValueError,
};
use serde::Deserialize;
use std::ops::Deref;

/// Contains definitions that can be referenced by other JSON structures though their `source`
/// fields, allowing repetition to be avoided and prevented.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Templates {
    /// List of templates for partially defining criteria structures.
    pub(crate) criteria_templates: Vec<CriteriaTemplate>,
    /// List of arrays that can be referenced by other lements.
    pub(crate) arrays: Vec<Array>,
    /// List of algorithms that can be referenced by [BucketBlock] elements.
    pub(crate) algorithms: Vec<Algorithm>,
    /// List of templates that can partially define [BucketBlock] elements.
    pub(crate) bucket_templates: Vec<BucketBlockTemplate>,
    /// List of waveforms that can be referenced by [BucketBlock] elements.
    pub(crate) waveforms: Vec<Waveform>,
}

impl Templates {
    /// Find the `BucketBlockTemplate` referenced in the given `BucketBlock`.
    pub(crate) fn get_bucket_block_template(
        &self,
        object: &BucketBlock,
    ) -> Option<&BucketBlockProperties> {
        self.bucket_templates
            .iter()
            .find_map(|tmplt| tmplt.is_source(object).then_some(tmplt.deref()))
    }

    /// Get a reference to the arrays.
    pub(crate) fn get_arrays(&self) -> &[Array] {
        &self.arrays
    }

    /// Find the `CriteriaTemplate` with the given name.
    pub(crate) fn get_criteria_template(&self, name: &str) -> Option<&CriteriaTemplate> {
        self.criteria_templates
            .iter()
            .find(|tmplt| tmplt.has_name(name))
    }

    pub(crate) fn get_algorithm(&self, name: &str) -> Option<&AlgorithmProperties> {
        self.algorithms
            .iter()
            .find_map(|alg| alg.has_name(name).then_some(alg.deref()))
    }

    pub(crate) fn get_waveform(&self, name: &str) -> Option<&WaveformProperties> {
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
    /// Optional name to save/load metric data.
    pub(crate) metrics_json_name: Option<String>,
    /// Topics the consumer should listen to to receive Eventlist messages.
    pub(crate) events_topics: Vec<String>,
    /// List of metrics to calculate and are available to the charts.
    pub(crate) metrics: Vec<Metric>,
    /// Templates of structures that are used when metrics, buckets, and charts are flattened.
    pub(crate) templates: Templates,
    /// Blocks of buckets that accept collections of eventlists.
    pub(crate) buckets: Vec<BucketBlock>,
    /// List of Charts.
    pub(crate) pages: Vec<Page>,
    /// Controls when to start the evaluation phase.
    pub(crate) idle_time_sec: i64,
}

impl AnalysisSettings {
    pub(crate) fn flatten_bucket_blocks(&self) -> Result<Vec<FlatBucketBlock>, BucketError> {
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

    pub(crate) fn flatten_pages(
        &self,
        buckets: &[FlatBucketBlock],
    ) -> Result<Vec<FlatPage>, PageError> {
        self.pages
            .iter()
            .map(|page| page.flatten((self, buckets)))
            .collect::<Result<_, PageError>>()
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

/// List of floating point values that can be used in `Function` structures.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Array {
    /// Name of the array.
    pub(crate) name: String,
    /// Is applied to all voltages when traces are created
    pub(crate) values: Vec<f64>,
}

impl Array {
    pub(crate) fn get_element(&self, index: usize) -> Option<f64> {
        self.values.get(index).copied()
    }
}

impl HasName for Array {
    fn get_name(&self) -> &str {
        &self.name
    }
}
