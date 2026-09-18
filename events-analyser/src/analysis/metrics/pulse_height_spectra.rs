use crate::{
    analysis::metrics::{
        MetricOutput, MetricResultError,
        output::HistogramWithBands,
        results::{CompleteMetricResultClass, PartialMetricResultClass},
        utils::{Histogram, SumWithSumOfSqrs},
    },
    engine::{
        FlatAlgorithm, FlatMetricPulseHeightSpectra, FlatWaveform, PulseHeightSpectraProperty,
    },
    eventlists::ChannelDataByTopic,
};
use digital_muon_common::Channel;
use num::Float;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Builds up histograms of pulse heights, for each channel.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PartialPulseHeightSpectra {
    /// Metric settings.
    source: FlatMetricPulseHeightSpectra,
    /// Per-channel histograms pulse heights.
    histogram: HashMap<Channel, Histogram>,
}

impl PartialMetricResultClass for PartialPulseHeightSpectra {
    type Source = FlatMetricPulseHeightSpectra;
    type Complete = CompletedPulseHeightSpectra;

    fn make_default(source: &FlatMetricPulseHeightSpectra) -> Self {
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
        for (_, intensity) in by_topic
            .get(self.source.topic)
            .expect("This should never fail.")
            .get_time_intensity()
        {
            self.histogram
                .entry(channel)
                .or_insert(Histogram::new(
                    self.source.histogram.num_bins,
                    &self.source.histogram.interval,
                ))
                .push(*intensity as f64);
        }
    }
}

/// Aggregates the pulse height histograms into a single histogram.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
}

impl CompleteMetricResultClass for CompletedPulseHeightSpectra {
    type Partial = PartialPulseHeightSpectra;
    type Error = MetricResultError;
    type Property = PulseHeightSpectraProperty;

    fn aggregate(source: &Self::Partial) -> Result<Self, Self::Error> {
        let labels = source
            .histogram
            .values()
            .next()
            .expect("No histogram values, this should never happen.") // FIXME: This might happen.
            .get_bin_labels()
            .to_vec();
        let mut sum = vec![0.0; labels.len()];
        let mut mean = vec![0.0; labels.len()];
        let mut sd = vec![0.0; labels.len()];
        let mut upper = vec![0.0; labels.len()];
        let mut lower = vec![f64::MAX; labels.len()];

        // Gather muta iterators of all the above vectors together (along with the index) and convert them to a convenient tuple form.
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

        Ok(CompletedPulseHeightSpectra {
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
            PulseHeightSpectraProperty::Sum => HistogramWithBands {
                labels: self.labels.clone(),
                central: self.sum.clone(),
                bands: None,
            },
            PulseHeightSpectraProperty::Mean => HistogramWithBands {
                labels: self.labels.clone(),
                central: self.mean.clone(),
                bands: None,
            },
            PulseHeightSpectraProperty::MeanSd => HistogramWithBands {
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
            PulseHeightSpectraProperty::MeanBounds => HistogramWithBands {
                labels: self.labels.clone(),
                central: self.sum.clone(),
                bands: Some((self.upper.clone(), self.lower.clone())),
            },
        };
        Ok(MetricOutput::Histograms(histogram))
    }
}
