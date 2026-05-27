use serde::Deserialize;

use crate::engine::Flattenable;

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "metric-type")]
pub(crate) enum Metric {
    FalseCount {
        name: String,
        true_topic: String,
        estimate_topic: String,
    },
}

impl Metric {
    pub(crate) fn has_name(&self, other_name: &str) -> bool {
        match self {
            Metric::FalseCount { name, .. } => name == other_name,
        }
    }
}

impl Flattenable for Metric {
    type Flat = FlatMetric;
    type Library = [String];
    type Error = String;

    fn flatten(&self, library: &Self::Library) -> Result<Self::Flat, Self::Error> {
        Ok(match self {
            Metric::FalseCount {
                name,
                true_topic,
                estimate_topic,
            } => FlatMetric::FalseCount(FlatMetricFalseCount {
                name: name.to_owned(),
                true_topic: library
                    .iter()
                    .enumerate()
                    .find_map(|(index, topic)| (topic == true_topic).then_some(index))
                    .ok_or_else(|| format!("Cannot find index of topic {true_topic}"))?,
                estimate_topic: library
                    .iter()
                    .enumerate()
                    .find_map(|(index, topic)| (topic == estimate_topic).then_some(index))
                    .ok_or_else(|| format!("Cannot find index of topic {estimate_topic}"))?,
            }),
        })
    }
}

pub(crate) enum FlatMetric {
    FalseCount(FlatMetricFalseCount),
}

pub(crate) struct FlatMetricFalseCount {
    pub(crate) name: String,
    pub(crate) true_topic: usize,
    pub(crate) estimate_topic: usize,
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "metric-type")]
pub(crate) struct FalseCount {}
