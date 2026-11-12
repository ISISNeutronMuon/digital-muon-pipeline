use crate::{
    engine::{
        Flattenable, HasName,
        values::{Interval, ValueError},
    },
    metric::{
        FlatMetricShell, FlatMetricType, JsonMetricShell, MetricType, flat, json, properties,
    },
};
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
