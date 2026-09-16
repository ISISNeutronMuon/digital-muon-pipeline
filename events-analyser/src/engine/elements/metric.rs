use crate::engine::{
    Flattenable, HasName,
    values::{Interval, ValueError},
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
        #[serde(flatten)]
        histogram: MetricTypeHistogram,
    },
    #[serde(rename_all = "kebab-case")]
    PulseHeightSpectra {
        topic: String,
        #[serde(flatten)]
        histogram: MetricTypeHistogram,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PropertyOfMetric {
    EventCount(EventCountProperty),
    FalseCount(FalseCountProperty),
    MuonLifetime(MuonLifetimeProperty),
    PulseHeightSpectra(PulseHeightSpectraProperty),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum EventCountProperty {
    TotalMean,
    TotalMeanWithSd,
    ChannelsBoxPlot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FalseCountProperty {
    TotalFalsePositivesMean,
    TotalFalsePositivesSd,
    TotalFalseNegativesMean,
    TotalFalseNegativesSd,
    TotalTruePositivesMean,
    TotalTruePositivesSd,
    TotalAmbiguousTruePositivesMean,
    TotalAmbiguousTruePositivesSd,
    ChannelsFalsePositivesBoxPlot,
    ChannelsFalseNegativesBoxPlot,
    ChannelsTruePositivesBoxPlot,
    ChannelsAmbiguousTruePositivesBoxPlot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MuonLifetimeProperty {
    TotalMean,
    TotalMeanWithSd,
    ChannelsBoxPlot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PulseHeightSpectraProperty {
    Sum,
    Mean,
    MeanSd,
    MeanBounds,
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
            MetricType::MuonLifetime { topic, histogram } => {
                FlatMetricType::MuonLifetime(FlatMetricMuonLifetime {
                    topic: library
                        .iter()
                        .enumerate()
                        .find_map(|(index, this_topic)| (this_topic == topic).then_some(index))
                        .expect("This should never fail."),
                    histogram: histogram.clone(),
                })
            }
            MetricType::PulseHeightSpectra { topic, histogram } => {
                FlatMetricType::PulseHeightSpectra(FlatMetricPulseHeightSpectra {
                    topic: library
                        .iter()
                        .enumerate()
                        .find_map(|(index, this_topic)| (this_topic == topic).then_some(index))
                        .expect("This should never fail."),
                    histogram: histogram.clone()
                })
            }
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
    PulseHeightSpectra(FlatMetricPulseHeightSpectra),
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
    #[serde(flatten)]
    pub(crate) histogram: MetricTypeHistogram,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatMetricPulseHeightSpectra {
    pub(crate) topic: usize,
    #[serde(flatten)]
    pub(crate) histogram: MetricTypeHistogram
}
