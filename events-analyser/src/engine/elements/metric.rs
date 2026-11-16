use crate::{
    engine::{
        Flattenable, HasName,
        values::{Interval, ValueError},
    },
    metric::{
        DependentMetricShell, ListOfMetrics, MetricWrapper, MetricWrapperWithTag,
        ResolvedMetricShell, flat, json, properties,
    },
};
use serde::{Deserialize, Serialize};

/// Groups together all metric Json types.
#[derive(Debug, Clone)]
pub(crate) struct JsonMetricTypes;

impl ListOfMetrics for JsonMetricTypes {
    type EventCount = DependentMetricShell<json::EventCount>;
    type FalseCount = DependentMetricShell<json::FalseCount>;
    type MuonLifetime = DependentMetricShell<json::MuonLifetime>;
    type PulseHeightSpectra = DependentMetricShell<json::PulseHeightSpectra>;
    type PulsePairTimeDiff = DependentMetricShell<json::PulsePairTimeDiff>;
}

/// Defines a metric type, should only be created by deserializing JSON object.
pub(crate) type MetricType = MetricWrapperWithTag<JsonMetricTypes>;

/// Groups together all metric properties types.
#[derive(Debug, Clone)]
pub(crate) struct PropertyMetricTypes;

impl ListOfMetrics for PropertyMetricTypes {
    type EventCount = properties::EventCount;
    type FalseCount = properties::FalseCount;
    type MuonLifetime = properties::MuonLifetime;
    type PulseHeightSpectra = properties::PulseHeightSpectra;
    type PulsePairTimeDiff = properties::PulsePairTimeDiff;
}

/// Defines a property of a metric, should only be created from deserializing JSON file.
pub(crate) type PropertyOfMetric = MetricWrapper<PropertyMetricTypes>;

/// Groups together all flat metric types.
#[derive(Debug, Clone)]
pub(crate) struct FlatMetricTypes;

impl ListOfMetrics for FlatMetricTypes {
    type EventCount = ResolvedMetricShell<flat::EventCount>;
    type FalseCount = ResolvedMetricShell<flat::FalseCount>;
    type MuonLifetime = ResolvedMetricShell<flat::MuonLifetime>;
    type PulseHeightSpectra = ResolvedMetricShell<flat::PulseHeightSpectra>;
    type PulsePairTimeDiff = ResolvedMetricShell<flat::PulsePairTimeDiff>;
}

/// Flattened definition of a metric, should only be created by calling [JsonMetricShell::flatten].
pub(crate) type FlatMetricType = MetricWrapper<FlatMetricTypes>;

impl Flattenable<&[String]> for MetricType {
    type Flat = FlatMetricType;
    type Error = ValueError;

    fn flatten(&self, library: &[String]) -> Result<Self::Flat, Self::Error> {
        Ok(match &self {
            MetricType::EventCount(json_metric_shell) => {
                FlatMetricType::EventCount(json_metric_shell.flatten(library)?)
            }
            MetricType::FalseCount(json_metric_shell) => {
                FlatMetricType::FalseCount(json_metric_shell.flatten(library)?)
            }
            MetricType::MuonLifetime(json_metric_shell) => {
                FlatMetricType::MuonLifetime(json_metric_shell.flatten(library)?)
            }
            MetricType::PulseHeightSpectra(json_metric_shell) => {
                FlatMetricType::PulseHeightSpectra(json_metric_shell.flatten(library)?)
            }
            MetricType::PulsePairTimeDiff(json_metric_shell) => {
                FlatMetricType::PulsePairTimeDiff(json_metric_shell.flatten(library)?)
            }
        })
    }
}

/// Defines a metric type, should only be created by deserializing JSON object.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Metric {
    /// Name of metric. This should be unique.
    pub(crate) name: String,
    /// Whether the metric's results should be saved in JSON format.
    #[serde(default)]
    pub(crate) save_to_json: bool,
    /// Defines the specific type of the metric.
    #[serde(flatten)]
    pub(crate) metric_type: MetricType,
}

impl HasName for Metric {
    fn get_name(&self) -> &str {
        &self.name
    }
}

/// Defines settings for a histogram type metric.
///
/// These do not require flattening.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct MetricTypeHistogram {
    /// Number of bins to use in the histogram.
    pub(crate) num_bins: usize,
    /// The upper and lower limits of the histogram.
    pub(crate) interval: Interval<f64>,
}

impl Flattenable<&[String]> for Metric {
    type Flat = FlatMetric;
    type Error = ValueError;

    fn flatten(&self, library: &[String]) -> Result<Self::Flat, Self::Error> {
        Ok(FlatMetric {
            name: self.get_name().to_string(),
            save_to_json: self.save_to_json,
            metric_type: self.metric_type.flatten(library)?,
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatMetric {
    pub(crate) name: String,
    pub(crate) save_to_json: bool,
    #[serde(flatten)]
    pub(crate) metric_type: FlatMetricType,
}

impl HasName for FlatMetric {
    fn get_name(&self) -> &str {
        &self.name
    }
}
