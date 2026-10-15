use crate::{
    analysis::metrics::{
        MetricOutput, MetricResultError,
        output::HistogramWithBands,
        results::{CompleteMetricResultBucket, PartialMetricResultBucket},
        utils::{Histogram, SumWithSumOfSqrs},
    },
    engine::{
        FlatAlgorithm, FlatMetricPulsePairDifferences, FlatWaveform, PulsePairDifferencesProperty,
    },
    eventlists::ChannelDataByTopic,
};
use digital_muon_common::{Channel, Time};
use itertools::Itertools;
use num::Float;
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
        let mut sum = vec![0.0; labels.len()];
        let mut mean = vec![0.0; labels.len()];
        let mut sd = vec![0.0; labels.len()];
        let mut upper = vec![0.0; labels.len()];
        let mut lower = vec![f64::MAX; labels.len()];

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
        for (index, sum, mean, sd, upper, lower) in zipped_iterators {
            let counts_iterator = source.histogram.values().map(|histogram| {
                histogram
                    .get_counts()
                    .get(index)
                    .expect("This should never fail.")
            });

            let mut sum_with_sum_of_sqrs = SumWithSumOfSqrs::default();
            for count in counts_iterator {
                sum_with_sum_of_sqrs.add_to(*count);
                *sum += count;
                *upper = upper.max(*count);
                *lower = lower.min(*count);
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
        })
    }

    fn get_property(&self, property: Self::Property) -> Result<MetricOutput, Self::Error> {
        let histogram = match property {
            PulsePairDifferencesProperty::Sum => HistogramWithBands {
                labels: self.labels.clone(),
                central: self.sum.clone(),
                bands: None,
            },
            PulsePairDifferencesProperty::Mean => HistogramWithBands {
                labels: self.labels.clone(),
                central: self.mean.clone(),
                bands: None,
            },
            PulsePairDifferencesProperty::MeanSd => HistogramWithBands {
                labels: self.labels.clone(),
                central: self.sum.clone(),
                bands: Some((
                    self.mean
                        .iter()
                        .zip(self.sd.iter())
                        .map(|(m, s)| m + s)
                        .collect(),
                    self.mean
                        .iter()
                        .zip(self.sd.iter())
                        .map(|(m, s)| m - s)
                        .collect(),
                )),
            },
            PulsePairDifferencesProperty::MeanBounds => HistogramWithBands {
                labels: self.labels.clone(),
                central: self.sum.clone(),
                bands: Some((self.upper.clone(), self.lower.clone())),
            },
        };
        Ok(MetricOutput::Histograms(histogram))
    }
}
