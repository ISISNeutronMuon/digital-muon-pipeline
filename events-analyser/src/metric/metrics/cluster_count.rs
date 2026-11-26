use crate::{
    engine::{Interval, ValueError},
    eventlists::ChannelDataByTopic,
    metric::{
        CompleteMetric, DependentMetric, PartialMetric, PartialMetricShell, ResolvedMetric,
        ResolvedMetricShell, error::MetricResultError, outputs::PartialSeriesOutput,
    },
};
use chrono::{DateTime, Utc};
use digital_muon_common::{Channel, FrameNumber, Intensity, Time};
use digital_muon_streaming_types::FrameMetadata;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Defines settings for the `ClusterCount` metric, that do not require resolving,
/// that is they do not depend on other items in the configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ClusterCountSettings {
    /// Number of pulses per cluster.
    cluster_size: usize,
    /// Maximum pairwise distance of pulses within a cluster.
    cluster_radius: usize,
    /// Record metadata with at least this many clusters.
    cluster_number: usize,
    /// Only include events with this range of intensities.
    intensity_filter: Option<Interval<Intensity>>,
}

/// Defines settings for the `ClusterCount` metric, that require resolving,
/// that is depend on other items defined in the configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct DependentClusterCount {
    /// The topic name to listen to for eventlist messages.
    topic: String,
}

impl DependentMetric for DependentClusterCount {
    type Resolved = ResolvedClusterCount;
    type Independent = ClusterCountSettings;

    fn resolve_metric(&self, library: &[String]) -> Result<Self::Resolved, ValueError> {
        Ok(ResolvedClusterCount {
            topic: library
                .iter()
                .enumerate()
                .find_map(|(index, this_topic)| (*this_topic == self.topic).then_some(index))
                .expect("This should never fail."),
        })
    }
}

/// Equivalent to settings in [DependentClusterCount], after their dependencies have been resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ResolvedClusterCount {
    /// The topic index to listen to for eventlist messages.
    topic: usize,
}

impl ResolvedMetric for ResolvedClusterCount {
    type Dependent = DependentClusterCount;
}

/// Encapsulates the clusters within a trace.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ClusterReport {
    /// Time the trace occurred.
    timestamp: DateTime<Utc>,
    /// Frame number of the trace.
    frame_number: FrameNumber,
    /// How many clusters were counted in the trace.
    cluster_count: usize,
}

/// Collects cluster reports as they are found.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PartialClusterCount {
    /// Per-channel list of cluster reports.
    cluster_reports: HashMap<Channel, Vec<ClusterReport>>,
}

impl PartialMetric for PartialClusterCount {
    type Source = ResolvedClusterCount;

    fn push(
        &mut self,
        source: &ResolvedMetricShell<Self::Source>,
        channel: Channel,
        metadata: &FrameMetadata,
        collection_by_topic: &ChannelDataByTopic,
    ) {
        // Tests whether the time difference of a pair if it is within the cluster radius, and at least one intensity is in range.
        let test_pair = |pair: Vec<&&(_, _)>| {
            let [(first_time, first_intensity), (last_time, last_intensity)] = pair
                .try_into()
                .expect("Vector should have length 2, this should never fail.");

            let is_in_filter = source.independent.intensity_filter.as_ref().is_none_or(
                |intensity_filter: &Interval<u16>| {
                    let range = intensity_filter.range_inclusive();
                    range.contains(first_intensity) || range.contains(last_intensity)
                },
            );

            let time_diff = Time::abs_diff(*first_time, *last_time);
            is_in_filter && time_diff <= source.independent.cluster_radius as Time
        };

        // Determines whether all pairs within the cluster pass `test_pair`.
        let test_cluster = |cluster: &Vec<&(_, _)>| cluster.iter().permutations(2).all(test_pair);

        let cluster_count = collection_by_topic
            .get(source.resolved.topic)
            .expect("This should never fail.")
            .get_time_intensity()
            .iter()
            .permutations(source.independent.cluster_size)
            .filter(test_cluster)
            .count();

        if cluster_count >= source.independent.cluster_number {
            // Obtain the channel's list of cluster reports.
            let cluster_reports = self.cluster_reports.entry(channel).or_default();
            cluster_reports.push(ClusterReport {
                timestamp: metadata.timestamp,
                frame_number: metadata.frame_number,
                cluster_count,
            })
        }
    }
}

/// Stores cluster reports.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct CompletedClusterCount {
    /// Histogram bin labels.
    cluster_reports: HashMap<Channel, Vec<ClusterReport>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ClusterCountProperty {
    /// Display a box plot for each bucket, plotted across the x-axis.
    BoxPlot,
}

impl CompleteMetric for CompletedClusterCount {
    type Partial = PartialClusterCount;
    type Error = MetricResultError;
    type Property = ClusterCountProperty;
    type Output = PartialSeriesOutput;

    fn aggregate(source: &PartialMetricShell<Self::Partial>) -> Result<Self, MetricResultError> {
        Ok(Self {
            cluster_reports: source.cluster_reports.clone(),
        })
    }

    fn get_property(&self, property: Self::Property) -> Result<Self::Output, Self::Error> {
        Ok(match property {
            Self::Property::BoxPlot => PartialSeriesOutput::LabelledGroup(
                self.cluster_reports
                    .iter()
                    .flat_map(|(channel, reports)| {
                        reports.iter().map(move |report| {
                            (
                                report.cluster_count as f64,
                                format!(
                                    "Frame: {}, DateTime: {}, Channel: {}",
                                    report.frame_number,
                                    report.timestamp.to_rfc3339(),
                                    channel
                                ),
                            )
                        })
                    })
                    .collect(),
            ),
        })
    }
}
