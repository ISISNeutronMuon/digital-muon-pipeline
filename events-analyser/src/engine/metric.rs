use serde::Deserialize;

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "metric-type")]
pub(crate) enum Metric {
    FalsePositiveCount {
        name: String,
        true_topic: String,
        estimate_topic: String,
    },
    FalseNegativeCount {
        name: String,
        true_topic: String,
        estimate_topic: String,
    },
}

impl Metric {
    pub(crate) fn has_name(&self, other_name: &str) -> bool {
        match self {
            Metric::FalsePositiveCount { name, .. } => name == other_name,
            Metric::FalseNegativeCount { name, .. } => name == other_name,
        }
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "metric-type")]
pub(crate) struct FalseCount {}
