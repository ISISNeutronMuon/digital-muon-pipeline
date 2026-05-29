use crate::engine::{Flattenable, values::ValueError};
use serde::Deserialize;

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "metric-type")]
pub(crate) enum Metric {
    FalseCount {
        true_topic: String,
        estimate_topic: String,
    },
}

impl Flattenable<&[String]> for Metric {
    type Flat = FlatMetric;
    type Error = ValueError;

    fn flatten(&self, library: &[String]) -> Result<Self::Flat, Self::Error> {
        Ok(match self {
            Metric::FalseCount {
                true_topic,
                estimate_topic,
            } => FlatMetric::FalseCount(FlatMetricFalseCount {
                true_topic: library
                    .iter()
                    .enumerate()
                    .find_map(|(index, topic)| (topic == true_topic).then_some(index))
                    .expect("This should never fail."),
                estimate_topic: library
                    .iter()
                    .enumerate()
                    .find_map(|(index, topic)| (topic == estimate_topic).then_some(index))
                    .expect("This should never fail."),
            }),
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) enum FlatMetric {
    FalseCount(FlatMetricFalseCount),
}

#[derive(Debug, Clone)]
pub(crate) struct FlatMetricFalseCount {
    pub(crate) true_topic: usize,
    pub(crate) estimate_topic: usize,
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "metric-type")]
pub(crate) struct FalseCount {}
