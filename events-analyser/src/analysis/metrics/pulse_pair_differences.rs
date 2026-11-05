use crate::{
    analysis::metrics::{
        MetricOutput, MetricResultError,
        output::{HistogramWithBands, Surface},
        results::{CompleteMetricResultBucket, PartialMetricResultBucket},
        utils::{Histogram, SumWithSumOfSqrs},
    },
    engine::{
        FlatAlgorithm, FlatMetricPulsePairDifferences, FlatWaveform, Interval, PulsePairDifferencesProperty
    },
    eventlists::ChannelDataByTopic,
};
use digital_muon_common::{Channel, Time};
use itertools::Itertools;
use num::{Float, Zero};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Builds up histograms of pulse heights, for each channel.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PartialPulsePairDifferences {
    /// Metric settings.
    source: FlatMetricPulsePairDifferences,
    /// Per-channel histograms pulse heights.
    histogram: HashMap<Channel, Histogram>,
}

impl PartialMetricResultBucket for PartialPulsePairDifferences {
    type Source = FlatMetricPulsePairDifferences;
    type Complete = CompletedPulsePairDifferences;

    fn make_default(source: &FlatMetricPulsePairDifferences) -> Self {
        Self {
            source: source.clone(),
            histogram: Default::default(),
        }
    }

    fn load_data(&mut self, source: &Self) {
        self.histogram = source.histogram.clone();
    }

    fn push(
        &mut self,
        _waveform: &FlatWaveform,
        _algorithm: &FlatAlgorithm,
        channel: Channel,
        by_topic: &ChannelDataByTopic,
    ) {
        // Filter the list of event times by event intensity.
        let times = by_topic
            .get(self.source.topic)
            .expect("This should never fail.")
            .get_time_intensity()
            .iter()
            .filter_map(|(time, intensity)| {
                self.source
                    .settings
                    .intensity_filter
                    .range_inclusive()
                    .contains(intensity)
                    .then_some(*time)
            })
            .collect::<Vec<_>>();

        // Calculate all temporal differences between each pair of events.
        let time_diffs = times.iter().permutations(2).map(|vec| {
            let [first, last] = vec
                .try_into()
                .expect("Vector should have length 2, this should never fail.");
            Time::abs_diff(*first, *last)
        });

        // Obtain the channel's histogram.
        let histogram = self
            .histogram
            .entry(channel)
            .or_insert_with(|| Histogram::new(&self.source.settings.histogram));

        // Enter the time differences into the histogram.
        for time_diff in time_diffs {
            histogram.push(time_diff as f64);
        }
    }
}

/// Aggregates the pulse height histograms into a single histogram.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CompletedPulsePairDifferences {
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
    surface: Vec<Vec<f64>>
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
fn get_normaliser(slice: &[f64], range: &Interval<usize>) -> Result<Option<f64>,()> {
    let len = range.get_max() as f64 - range.get_min() as f64;
    if len.is_sign_negative() && len.is_zero() {
        return Err(())
    }
    let mean = slice.iter().take(range.get_max()).skip(range.get_min()).sum::<f64>()/len;
    if mean.is_zero() {
        return Ok(None)
    }
    Ok(Some(1.0/mean))
}

impl CompleteMetricResultBucket for CompletedPulsePairDifferences {
    type Partial = PartialPulsePairDifferences;
    type Error = MetricResultError;
    type Property = PulsePairDifferencesProperty;

    fn aggregate(source: &Self::Partial) -> Result<Self, Self::Error> {
        let labels = source
            .histogram
            .values()
            .next()
            .map(|hist| hist.get_bin_labels().to_vec())
            .unwrap_or_else(|| Histogram::generate_bin_labels(&source.source.settings.histogram));
        let histograms = source.histogram.iter().sorted_by(|(a,_),(b,_)|u32::cmp(a,b)).collect::<Vec<_>>();
        let channels = histograms.iter().map(|(c,_)|**c as f64).collect::<Vec<_>>();

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
        let histograms_with_normalisers = histograms.iter().map(|&(_, histogram)| {
            let normaliser = get_normaliser(histogram.get_counts(), &source.source.settings.normaliser)
                .expect("This should be pre-validated, this should never fail.");
            (histogram, normaliser)
        }).collect::<Vec<_>>();

        // Compute statistics.
        for (time_index, sum, mean, sd, upper, lower) in zipped_iterators {
            // Iterator that loops through each normalised histogram's value at `index`.
            let counts = histograms_with_normalisers.iter()
                .map(|&(histogram, normaliser)| {
                    if let Some(normaliser) = normaliser {
                        normaliser*histogram
                            .get_counts()
                            .get(time_index)
                            .expect("This should never fail.")
                    } else {
                        0.0 // FIXME: Should we do something else in this case?
                    }
                }
            ).collect::<Vec<_>>();

            for (channel_index, count) in counts.iter().enumerate() {
                surface[channel_index][time_index] = *count
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

        Ok(CompletedPulsePairDifferences {
            labels,
            mean,
            sd,
            sum,
            upper,
            lower,
            channels,
            surface,
        })
    }

    fn get_property(&self, property: Self::Property) -> Result<MetricOutput, Self::Error> {
        let metric_output = match property {
            PulsePairDifferencesProperty::Sum => MetricOutput::Histograms(HistogramWithBands {
                labels: self.labels.clone(),
                central: self.sum.clone(),
                bands: None,
            }),
            PulsePairDifferencesProperty::Mean => MetricOutput::Histograms(HistogramWithBands {
                labels: self.labels.clone(),
                central: self.mean.clone(),
                bands: None,
            }),
            PulsePairDifferencesProperty::MeanSd => MetricOutput::Histograms(HistogramWithBands {
                labels: self.labels.clone(),
                central: self.mean.clone(),
                bands: Some((self.sd.clone(), self.sd.clone())),
            }),
            PulsePairDifferencesProperty::MeanBounds => MetricOutput::Histograms(HistogramWithBands {
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
            PulsePairDifferencesProperty::ChannelHeatMap => MetricOutput::Surface(
                Surface {
                    x_values: self.labels.clone(),
                    y_values: self.channels.clone(),
                    data: self.surface.clone()
                })
        };
        Ok(metric_output)
    }
}
