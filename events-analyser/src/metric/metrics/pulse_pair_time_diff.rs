use crate::{
    engine::{Interval, MetricTypeHistogram, ValueError},
    eventlists::ChannelDataByTopic,
    metric::{CompleteMetric, FlattenedMetric, JsonMetric, PartialMetric,
        error::MetricResultError, FlatMetricShell, PartialMetricShell,
        outputs::{HistogramWithBands, CompleteSeriesOutput, Surface},
        utils::{Histogram, SumWithSumOfSqrs}
    },
};
use digital_muon_common::{Channel, Intensity, Time};
use itertools::Itertools;
use num::{Float, Zero};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PulsePairTimeDiffSettings {
    /// Only include events with this range of intensities.
    intensity_filter: Interval<Intensity>,
    /// Encapsulates how the histogram is constructed.
    #[serde(flatten)]
    histogram: MetricTypeHistogram,
    /// Normalise the histogram by the average of values from this interval.
    normaliser: Interval<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct JsonPulsePairTimeDiff {
    topic: String,
}

impl JsonMetric for JsonPulsePairTimeDiff {
    type Flattened = FlatPulsePairTimeDiff;
    type Settings = PulsePairTimeDiffSettings;

    fn flatten_metric(&self, library: &[String]) -> Result<Self::Flattened, ValueError> {
        Ok(FlatPulsePairTimeDiff {
            topic: library
                .iter()
                .enumerate()
                .find_map(|(index, this_topic)| (*this_topic == self.topic).then_some(index))
                .expect("This should never fail."),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatPulsePairTimeDiff {
    topic: usize,
}

impl FlattenedMetric for FlatPulsePairTimeDiff {
    type Json = JsonPulsePairTimeDiff;
}

/// Builds up histograms of pulse heights, for each channel.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PartialPulsePairTimeDiff {
    /// Per-channel histograms pulse heights.
    histograms: HashMap<Channel, Histogram>,
}

impl PartialMetric for PartialPulsePairTimeDiff {
    type Source = FlatPulsePairTimeDiff;

    fn push(
        &mut self,
        source: &FlatMetricShell<Self::Source>,
        channel: Channel,
        collection_by_topic: &ChannelDataByTopic,
    ) {
        let time_diffs = collection_by_topic
            .get(source.flat.topic)
            .expect("This should never fail.")
            .get_time_intensity()
            .iter()
            .permutations(2)
            .filter_map(|vec| {
                let [(first_time, first_intensity), (last_time, last_intensity)] = vec
                    .try_into()
                    .expect("Vector should have length 2, this should never fail.");
                let range = source.settings.intensity_filter.range_inclusive();
                (range.contains(first_intensity) || range.contains(last_intensity))
                    .then_some(Time::abs_diff(*first_time, *last_time))
            })
            .collect::<Vec<_>>();

        // Obtain the channel's histogram.
        let histogram = self
            .histograms
            .entry(channel)
            .or_insert_with(|| Histogram::new(&source.settings.histogram));

        // Enter the time differences into the histogram.
        for time_diff in time_diffs {
            histogram.push(time_diff as f64);
        }
    }
}

/// Aggregates the pulse height histograms into a single histogram.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct CompletedPulsePairTimeDiff {
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

/// Calculates the inverse of the mean of the given interval of the slice.
///
/// # Return
/// - If the calculated `mean` is non-zero, returns `Ok(Some(1.0/mean))`
/// - otherwise, returns `Ok(None)`.
/// - If the given interval is not a valid and of nonzero length, then returns an error.
///
/// # Parameters
///  - slice: the slice to mutate.
///  - range: the range of the slice to take the mean of.
fn get_normaliser(slice: &[f64], range: &Interval<usize>) -> Result<Option<f64>, ()> {
    let len = range.get_max() as f64 - range.get_min() as f64;
    if len.is_sign_negative() && len.is_zero() {
        return Err(());
    }
    let mean = slice
        .iter()
        .take(range.get_max())
        .skip(range.get_min())
        .sum::<f64>()
        / len;
    if mean.is_zero() {
        return Ok(None);
    }
    Ok(Some(1.0 / mean))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PulsePairTimeDiffProperty {
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

impl CompleteMetric for CompletedPulsePairTimeDiff {
    type Partial = PartialPulsePairTimeDiff;
    type Error = MetricResultError;
    type Property = PulsePairTimeDiffProperty;
    type Output = CompleteSeriesOutput;

    fn aggregate(source: &PartialMetricShell<Self::Partial>) -> Result<Self, MetricResultError> {
        let labels = source
            .histograms
            .values()
            .next()
            .map(|hist| hist.get_bin_labels().to_vec())
            .unwrap_or_else(|| Histogram::generate_bin_labels(&source.source.settings.histogram));
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
        let histograms_with_normalisers = histograms
            .iter()
            .map(|&(_, histogram)| {
                let normaliser =
                    get_normaliser(histogram.get_counts(), &source.source.settings.normaliser)
                        .expect("This should be pre-validated, this should never fail.");
                (histogram, normaliser)
            })
            .collect::<Vec<_>>();

        // Compute statistics.
        for (time_index, sum, mean, sd, upper, lower) in zipped_iterators {
            // Iterator that loops through each normalised histogram's value at `index`.
            let counts = histograms_with_normalisers
                .iter()
                .map(|&(histogram, normaliser)| {
                    if let Some(normaliser) = normaliser {
                        normaliser
                            * histogram
                                .get_counts()
                                .get(time_index)
                                .expect("This should never fail.")
                    } else {
                        0.0 // FIXME: Should we do something else in this case?
                    }
                })
                .collect::<Vec<_>>();

            for (channel_index, count) in counts.iter().enumerate() {
                *surface
                    .get_mut(channel_index)
                    .expect("Element should exist, this should never fail.")
                    .get_mut(time_index)
                    .expect("Element should exist, this should never fail.") = *count
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

        Ok(CompletedPulsePairTimeDiff {
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
