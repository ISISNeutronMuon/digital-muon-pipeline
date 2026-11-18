use crate::{
    engine::{Interval, MetricTypeHistogram, ValueError},
    eventlists::ChannelDataByTopic,
    metric::{
        CompleteMetric, DependentMetric, PartialMetric, PartialMetricShell, ResolvedMetric,
        ResolvedMetricShell,
        error::MetricResultError,
        outputs::{CompleteSeriesOutput, HistogramWithBands, Surface},
        utils::{Histogram, SumWithSumOfSqrs},
    },
};
use digital_muon_common::{Channel, Intensity, Time};
use itertools::Itertools;
use num::Float;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Defines settings for the `PulsePairTimeDiff` metric, that do not require resolving,
/// that is they do not depend on other items in the configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ClusterCountSettings {
    /// Number of pulses per cluster.
    cluster_size: usize,
    /// Maximum pairwise distance of pulses within a cluster.
    cluster_radius: usize,
    /// Only include events with this range of intensities.
    intensity_filter: Option<Interval<Intensity>>,
    /// Encapsulates how the histogram is constructed.
    #[serde(flatten)]
    histogram: MetricTypeHistogram,
}

/// Defines settings for the `PulsePairTimeDiff` metric, that require resolving,
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

/// Equivalent to settings in [DependentPulsePairTimeDiff], after their dependencies have been resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ResolvedClusterCount {
    /// The topic index to listen to for eventlist messages.
    topic: usize,
}

impl ResolvedMetric for ResolvedClusterCount {
    type Dependent = DependentClusterCount;
}

/// Builds up histograms of pulse heights, for each channel.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PartialClusterCount {
    /// Per-channel histograms pulse heights.
    histograms: HashMap<Channel, Histogram>,
}

impl PartialMetric for PartialClusterCount {
    type Source = ResolvedClusterCount;

    fn push(
        &mut self,
        source: &ResolvedMetricShell<Self::Source>,
        channel: Channel,
        collection_by_topic: &ChannelDataByTopic,
    ) {
        let time_diffs = collection_by_topic
            .get(source.resolved.topic)
            .expect("This should never fail.")
            .get_time_intensity()
            .iter()
            .permutations(source.independent.cluster_size)
            .filter_map(|cluster| {
                cluster.into_iter()
                    .permutations(2)
                    .filter_map(|pair| {
                        let [(first_time, first_intensity), (last_time, last_intensity)] = pair
                            .try_into()
                            .expect("Vector should have length 2, this should never fail.");
                        let is_in_filter = source
                            .independent
                            .intensity_filter
                            .as_ref()
                            .is_none_or(|intensity_filter| {
                                let range = intensity_filter.range_inclusive();
                                range.contains(first_intensity) || range.contains(last_intensity)
                            });
                        let time_diff = Time::abs_diff(*first_time, *last_time);
                        (is_in_filter && time_diff <= source.independent.cluster_radius as Time)
                        .then_some(time_diff)
                    })
                    .min()
            })
            .collect::<Vec<_>>();

        // Obtain the channel's histogram.
        let histogram = self
            .histograms
            .entry(channel)
            .or_insert_with(|| Histogram::new(&source.independent.histogram));

        // Enter the time differences into the histogram.
        for time_diff in time_diffs {
            histogram.push(time_diff as f64);
        }
    }
}

/// Aggregates the pulse height histograms into a single histogram.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct CompletedClusterCount {
    /// Histogram bin labels.
    labels: Vec<f64>,
    /// Values of the histogram formed by summing all per-channel histograms together.
    sum: Vec<f64>,
    /// Values of the histogram formed by taking the arithmetic mean of all per-channel histograms.
    mean: Vec<f64>,
    /// Standard deviations of the histogram values formed by taking the arithmetic mean of all per-channel histograms.
    sd: Vec<f64>,
    /// Values of the histogram formed by taking the maximum of all per-channel histograms.
    upper: Vec<f64>,
    /// Values of the histogram formed by taking the minimum of all per-channel histograms.
    lower: Vec<f64>,
    /// Channels.
    channels: Vec<f64>,
    /// Two dimensional surface of the histogram values of all channels.
    surface: Vec<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ClusterCountProperty {
    /// Display histogram whose counts are the sums of the per-channel counts.
    Sum,
    /// Display histogram whose counts are the mean of the per-channel counts.
    Mean,
    /// Display histogram whose counts are the mean of the per-channel counts, along with the standard deviation bands.
    MeanSd,
    /// Display histogram whose counts are the mean of the per-channel counts, along with the minimum and maximum count bands.
    MeanBounds,
    /// Display counts as a per-channel heatmap.
    ChannelHeatMap,
}

impl CompleteMetric for CompletedClusterCount {
    type Partial = PartialClusterCount;
    type Error = MetricResultError;
    type Property = ClusterCountProperty;
    type Output = CompleteSeriesOutput;

    fn aggregate(source: &PartialMetricShell<Self::Partial>) -> Result<Self, MetricResultError> {
        let labels = source
            .histograms
            .values()
            .next()
            .map(|hist| hist.get_bin_labels().to_vec())
            .unwrap_or_else(|| {
                Histogram::generate_bin_labels(&source.source.independent.histogram)
            });
        let histograms = source
            .histograms
            .iter()
            .sorted_by(|(a, _), (b, _)| u32::cmp(a, b))
            .collect::<Vec<_>>();
        let channels = histograms
            .iter()
            .map(|(c, _)| **c as f64)
            .collect::<Vec<_>>();

        let mut sum = vec![0.0; labels.len()];
        let mut mean = vec![0.0; labels.len()];
        let mut sd = vec![0.0; labels.len()];
        let mut upper = vec![0.0; labels.len()];
        let mut lower = vec![f64::MAX; labels.len()];
        let mut surface = vec![vec![0.0; labels.len()]; channels.len()];
        //let mut surface = Vec::<f64>::with_capacity(labels.len()*source.histogram.len());

        // Gather mut iterators of all the above vectors together (along with the index) and convert them to a convenient tuple form.
        let zipped_iterators = sum
            .iter_mut()
            .enumerate()
            .zip(mean.iter_mut())
            .zip(sd.iter_mut())
            .zip(upper.iter_mut())
            .zip(lower.iter_mut())
            .map(|(((((index, sum), mean), sd), upper), lower)| {
                (index, sum, mean, sd, upper, lower)
            });

        // Collect channel histograms and calculate their normalisers.
        // let histograms_with_normalisers = histograms
        //     .iter()
        //     .map(|&(_, histogram)| {
        //         let normaliser = get_normaliser(
        //             histogram.get_counts(),
        //             &source.source.independent.normaliser,
        //         )
        //         .expect("This should be pre-validated, this should never fail.");
        //         (histogram, normaliser)
        //     })
        //     .collect::<Vec<_>>();

        // Compute statistics.
        for (time_index, sum, mean, sd, upper, lower) in zipped_iterators {
            // Iterator that loops through each normalised histogram's value at `index`.
            let counts = histograms
                .iter()
                .map(|&(_, histogram)| {
                    histogram
                        .get_counts()
                        .get(time_index)
                        .expect("This should never fail.")
                })
                .cloned()
                .collect::<Vec<_>>();

            for (channel_index, &count) in counts.iter().enumerate() {
                *surface
                    .get_mut(channel_index)
                    .expect("Element should exist, this should never fail.")
                    .get_mut(time_index)
                    .expect("Element should exist, this should never fail.") = count
            }

            let mut sum_with_sum_of_sqrs = SumWithSumOfSqrs::default();
            for count in counts {
                sum_with_sum_of_sqrs.add_to(count);
                *sum += count;
                *upper = upper.max(count);
                *lower = lower.min(count);
            }
            let mean_sd = sum_with_sum_of_sqrs.mean_and_stddev();
            *mean = mean_sd.mean;
            *sd = mean_sd.sd;
        }

        Ok(CompletedClusterCount {
            labels,
            sum,
            mean,
            sd,
            upper,
            lower,
            channels,
            surface,
        })
    }

    fn get_property(&self, property: Self::Property) -> Result<Self::Output, Self::Error> {
        Ok(match property {
            Self::Property::Sum => CompleteSeriesOutput::Histogram(HistogramWithBands {
                labels: self.labels.clone(),
                central: self.sum.clone(),
                bands: None,
            }),
            Self::Property::Mean => CompleteSeriesOutput::Histogram(HistogramWithBands {
                labels: self.labels.clone(),
                central: self.mean.clone(),
                bands: None,
            }),
            Self::Property::MeanSd => CompleteSeriesOutput::Histogram(HistogramWithBands {
                labels: self.labels.clone(),
                central: self.mean.clone(),
                bands: Some((self.sd.clone(), self.sd.clone())),
            }),
            Self::Property::MeanBounds => CompleteSeriesOutput::Histogram(HistogramWithBands {
                labels: self.labels.clone(),
                central: self.sum.clone(),
                bands: Some((
                    self.upper
                        .iter()
                        .zip(self.sum.iter())
                        .map(|(upper, sum)| upper - sum)
                        .collect(),
                    self.lower
                        .iter()
                        .zip(self.sd.iter())
                        .map(|(lower, sum)| sum - lower)
                        .collect(),
                )),
            }),
            Self::Property::ChannelHeatMap => CompleteSeriesOutput::HeatMap(Surface {
                x_values: self.labels.clone(),
                y_values: self.channels.clone(),
                data: self.surface.clone(),
            }),
        })
    }
}
