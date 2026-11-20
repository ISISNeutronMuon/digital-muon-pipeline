use crate::{
    engine::{MetricTypeHistogram, ValueError},
    eventlists::ChannelDataByTopic,
    metric::{
        CompleteMetric, DependentMetric, PartialMetric, PartialMetricShell, ResolvedMetric,
        ResolvedMetricShell,
        error::MetricResultError,
        outputs::{CompleteSeriesOutput, HistogramWithBands, Surface},
        utils::{Histogram, SumWithSumOfSqrs},
    },
};
use digital_muon_common::Channel;
use itertools::Itertools;
use num::Float;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Defines settings for the `PulseHeightSpectra` metric, that require resolving,
/// that is depend on other items defined in the configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct DependentPulseHeightSpectra {
    /// The topic name to listen to for eventlist messages.
    topic: String,
}

impl DependentMetric for DependentPulseHeightSpectra {
    type Resolved = ResolvedPulseHeightSpectra;
    type Independent = MetricTypeHistogram;

    fn resolve_metric(&self, library: &[String]) -> Result<Self::Resolved, ValueError> {
        Ok(ResolvedPulseHeightSpectra {
            topic: library
                .iter()
                .enumerate()
                .find_map(|(index, this_topic)| (*this_topic == self.topic).then_some(index))
                .expect("This should never fail."),
        })
    }
}

/// Equivalent to settings in [DependentPulseHeightSpectra], after their dependencies have been resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ResolvedPulseHeightSpectra {
    /// The topic index to listen to for eventlist messages.
    topic: usize,
}

impl ResolvedMetric for ResolvedPulseHeightSpectra {
    type Dependent = DependentPulseHeightSpectra;
}

/// Builds up histograms of pulse heights, for each channel.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PartialPulseHeightSpectra {
    /// Per-channel histograms pulse heights.
    histograms: HashMap<Channel, Histogram>,
}

impl PartialMetric for PartialPulseHeightSpectra {
    type Source = ResolvedPulseHeightSpectra;

    fn push(
        &mut self,
        source: &ResolvedMetricShell<Self::Source>,
        channel: Channel,
        collection_by_topic: &ChannelDataByTopic,
    ) {
        for (_, intensity) in collection_by_topic
            .get(source.topic)
            .expect("This should never fail.")
            .get_time_intensity()
        {
            self.histograms
                .entry(channel)
                .or_insert(Histogram::new(&source.independent))
                .push(*intensity as f64);
        }
    }
}

/// Aggregates the pulse height histograms into a single histogram.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct CompletedPulseHeightSpectra {
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
pub(crate) enum PulseHeightSpectraProperty {
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

impl CompleteMetric for CompletedPulseHeightSpectra {
    type Partial = PartialPulseHeightSpectra;
    type Error = MetricResultError;
    type Property = PulseHeightSpectraProperty;
    type Output = CompleteSeriesOutput;

    fn aggregate(source: &PartialMetricShell<Self::Partial>) -> Result<Self, MetricResultError> {
        let labels = source
            .histograms
            .values()
            .next()
            .map(|hist| hist.get_bin_labels().to_vec())
            .unwrap_or_else(|| Histogram::generate_bin_labels(&source.source.independent));
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

        // Compute statistics.
        for (time_index, sum, mean, sd, upper, lower) in zipped_iterators {
            // Iterator that loops through each normalised histogram's value at `index`.

            let counts_iterator = source.histograms.values().map(|histogram| {
                histogram
                    .get_counts()
                    .get(time_index)
                    .expect("This should never fail.")
            });

            let mut sum_with_sum_of_sqrs = SumWithSumOfSqrs::default();
            for (channel_index, count) in counts_iterator.enumerate() {
                *surface
                    .get_mut(channel_index)
                    .expect("Element should exist, this should never fail.")
                    .get_mut(time_index)
                    .expect("Element should exist, this should never fail.") = *count;

                sum_with_sum_of_sqrs.add_to(*count);
                *sum += count;
                *upper = upper.max(*count);
                *lower = lower.min(*count);
            }
            let mean_sd = sum_with_sum_of_sqrs.mean_and_stddev();
            *mean = mean_sd.mean;
            *sd = mean_sd.sd;
        }

        Ok(Self {
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
