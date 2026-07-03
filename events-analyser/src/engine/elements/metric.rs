use crate::engine::{Flattenable, HasName, values::ValueError};
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

impl Metric {
    pub(crate) fn get_property(&self, property: &str) -> Result<MetricProperty, MetricError> {
        self.metric_type.get_property(property)
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "metric-type")]
pub(crate) enum MetricType {
    #[serde(rename_all = "kebab-case")]
    EventCount { topic: String },
    #[serde(rename_all = "kebab-case")]
    FalseCount {
        true_topic: String,
        estimate_topic: String,
    },
    #[serde(rename_all = "kebab-case")]
    MuonLifetime {
        topic: String,
        num_bins: usize,
        max_lifetime: f64,
    },
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
    TruePositivesMean,
    TruePositivesSD,
    AmbiguousTruePositivesMean,
    AmbiguousTruePositivesSD,
}

impl MetricType {
    pub(crate) fn get_property(&self, property: &str) -> Result<MetricProperty, MetricError> {
        match self {
            Self::EventCount { .. } => match property {
                "mean" => Ok(MetricProperty::Mean),
                "sd" => Ok(MetricProperty::SD),
                _ => Err(MetricError::NoProperty(
                    property.to_string(),
                    "Event Count".into(),
                )),
            },
            Self::FalseCount { .. } => match property {
                "false-positives-mean" => Ok(MetricProperty::FalsePositivesMean),
                "false-positives-sd" => Ok(MetricProperty::FalsePositivesSD),
                "false-negatives-mean" => Ok(MetricProperty::FalseNegativesMean),
                "false-negatives-sd" => Ok(MetricProperty::FalseNegativesSD),
                "true-positives-mean" => Ok(MetricProperty::TruePositivesMean),
                "true-positives-sd" => Ok(MetricProperty::TruePositivesSD),
                "ambiguous-true-positives-mean" => Ok(MetricProperty::AmbiguousTruePositivesMean),
                "ambiguous-true-positives-sd" => Ok(MetricProperty::AmbiguousTruePositivesSD),
                _ => Err(MetricError::NoProperty(
                    property.to_string(),
                    "False Count".into(),
                )),
            },
            Self::MuonLifetime { .. } => match property {
                "mean" => Ok(MetricProperty::Mean),
                "sd" => Ok(MetricProperty::SD),
                _ => Err(MetricError::NoProperty(
                    property.to_string(),
                    "Muon Lifetime".into(),
                )),
            },
        }
    }
}

impl Flattenable<&[String]> for Metric {
    type Flat = FlatMetric;
    type Error = ValueError;

    fn flatten(&self, library: &[String]) -> Result<Self::Flat, Self::Error> {
        let metric_type = match &self.metric_type {
            MetricType::EventCount { topic } => FlatMetricType::EventCount(FlatMetricEventCount {
                topic: library
                    .iter()
                    .enumerate()
                    .find_map(|(index, this_topic)| (this_topic == topic).then_some(index))
                    .expect("This should never fail."),
            }),
            MetricType::FalseCount {
                true_topic,
                estimate_topic,
            } => FlatMetricType::FalseCount(FlatMetricFalseCount {
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
            MetricType::MuonLifetime {
                topic,
                num_bins,
                max_lifetime,
            } => FlatMetricType::MuonLifetime(FlatMetricMuonLifetime {
                topic: library
                    .iter()
                    .enumerate()
                    .find_map(|(index, this_topic)| (this_topic == topic).then_some(index))
                    .expect("This should never fail."),
                num_bins: *num_bins,
                max_lifetime: *max_lifetime,
            }),
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
    EventCount(FlatMetricEventCount),
    FalseCount(FlatMetricFalseCount),
    MuonLifetime(FlatMetricMuonLifetime),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatMetricFalseCount {
    pub(crate) true_topic: usize,
    pub(crate) estimate_topic: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FlatMetricEventCount {
    pub(crate) topic: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatMetricMuonLifetime {
    pub(crate) topic: usize,
    pub(crate) num_bins: usize,
    pub(crate) max_lifetime: f64,
}
