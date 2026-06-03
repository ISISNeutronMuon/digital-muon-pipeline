use crate::engine::{Flattenable, values::ValueError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum MetricError {
    #[error("Property not found {0} for Metric {1}.")]
    NoProperty(String, String),
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "metric-type")]
pub(crate) enum Metric {
    EventCount {
        topic: String,
    },
    FalseCount {
        true_topic: String,
        estimate_topic: String,
    },
    MuonLifetime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MetricProperty {
    Mean,
    SD,
    FalsePositivesMean,
    FalsePositivesSD,
    FalseNegativesMean,
    FalseNegativesSD,
}

impl Metric {
    pub(crate) fn get_property(&self, property: &str) -> Result<MetricProperty, MetricError> {
        match self {
            Self::EventCount { .. } => match property {
                "mean" => Ok(MetricProperty::Mean),
                "sd" => Ok(MetricProperty::SD),
                _ => Err(MetricError::NoProperty(property.to_string(), "Event Count".into())),
            },
            Self::FalseCount { .. } => match property {
                "false-positives-mean" => Ok(MetricProperty::FalsePositivesMean),
                "false-positives-sd" => Ok(MetricProperty::FalsePositivesSD),
                "false-negatives-mean" => Ok(MetricProperty::FalseNegativesMean),
                "false-negatives-sd" => Ok(MetricProperty::FalseNegativesSD),
                _ => Err(MetricError::NoProperty(property.to_string(), "False Count".into())),
            },
            Self::MuonLifetime => match property {
                "mean" => Ok(MetricProperty::Mean),
                "sd" => Ok(MetricProperty::SD),
                _ => Err(MetricError::NoProperty(property.to_string(), "Muon Lifetime".into())),
            },
        }
    }
}

impl Flattenable<&[String]> for Metric {
    type Flat = FlatMetric;
    type Error = ValueError;

    fn flatten(&self, library: &[String]) -> Result<Self::Flat, Self::Error> {
        Ok(match self {
            Metric::EventCount {
                topic,
            } => FlatMetric::EventCount(FlatMetricEventCount {
                topic: library
                    .iter()
                    .enumerate()
                    .find_map(|(index, this_topic)| (this_topic == topic).then_some(index))
                    .expect("This should never fail.")
            }),
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
            Metric::MuonLifetime => FlatMetric::MuonLifetime,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) enum FlatMetric {
    EventCount(FlatMetricEventCount),
    FalseCount(FlatMetricFalseCount),
    MuonLifetime,
}

#[derive(Debug, Clone)]
pub(crate) struct FlatMetricFalseCount {
    pub(crate) true_topic: usize,
    pub(crate) estimate_topic: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct FlatMetricEventCount {
    pub(crate) topic: usize,
}