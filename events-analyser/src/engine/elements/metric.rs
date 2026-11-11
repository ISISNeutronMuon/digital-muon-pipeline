use crate::{engine::{
    Flattenable, HasName,
    values::{Interval, ValueError},
}, metric::{FlatMetricShell, JsonMetricShell, flat, json, properties}};
use serde::{Deserialize, Serialize};

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Metric {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) save_to_json: bool,
    #[serde(flatten)]
    pub(crate) metric_type: MetricType,
}

impl HasName for Metric {
    fn get_name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct MetricTypeHistogram {
    pub(crate) num_bins: usize,
    pub(crate) interval: Interval<f64>,
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "metric-type")]
pub(crate) enum MetricType {
    #[serde(rename_all = "kebab-case")]
    EventCount(JsonMetricShell<json::EventCount>),
    #[serde(rename_all = "kebab-case")]
    FalseCount(JsonMetricShell<json::FalseCount>),
    #[serde(rename_all = "kebab-case")]
    MuonLifetime(JsonMetricShell<json::MuonLifetime>),
    #[serde(rename_all = "kebab-case")]
    PulseHeightSpectra(JsonMetricShell<json::PulseHeightSpectra>),
    #[serde(rename_all = "kebab-case")]
    PulsePairDifferences(JsonMetricShell<json::PulsePairTimeDiff>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PropertyOfMetric {
    EventCount(properties::EventCount),
    FalseCount(properties::FalseCount),
    MuonLifetime(properties::MuonLifetime),
    PulseHeightSpectra(properties::PulseHeightSpectra),
    PulsePairDifferences(properties::PulsePairTimeDiff),
}

impl Flattenable<&[String]> for Metric {
    type Flat = FlatMetric;
    type Error = ValueError;

    fn flatten(&self, library: &[String]) -> Result<Self::Flat, Self::Error> {
        let metric_type = match &self.metric_type {
            MetricType::EventCount(json_metric_shell) => FlatMetricType::EventCount(json_metric_shell.flatten(library)?),
            MetricType::FalseCount(json_metric_shell) => FlatMetricType::FalseCount(json_metric_shell.flatten(library)?),
            MetricType::MuonLifetime(json_metric_shell) => FlatMetricType::MuonLifetime(json_metric_shell.flatten(library)?),
            MetricType::PulseHeightSpectra(json_metric_shell) => FlatMetricType::PulseHeightSpectra(json_metric_shell.flatten(library)?),
            MetricType::PulsePairDifferences(json_metric_shell) => FlatMetricType::PulsePairDifferences(json_metric_shell.flatten(library)?),
        };
        Ok(FlatMetric {
            name: self.get_name().to_string(),
            save_to_json: self.save_to_json,
            metric_type,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FlatMetricType {
    EventCount(FlatMetricShell<flat::EventCount>),
    FalseCount(FlatMetricShell<flat::FalseCount>),
    MuonLifetime(FlatMetricShell<flat::MuonLifetime>),
    PulseHeightSpectra(FlatMetricShell<flat::PulseHeightSpectra>),
    PulsePairDifferences(FlatMetricShell<flat::PulsePairTimeDiff>),
}