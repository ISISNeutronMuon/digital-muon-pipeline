use crate::{
    analysis::metrics::{
        MetricOutput, MetricResultError,
        results::{CompleteMetricResultBucket, PartialMetricResultBucket},
        utils::{GroupDataBy, MeanSD, SumWithSumOfSqrs},
    },
    engine::{FalseCountProperty, FlatAlgorithm, FlatMetricFalseCount, FlatWaveform},
    event::ChannelData,
    eventlists::ChannelDataByTopic,
};
use digital_muon_common::Channel;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{collections::HashMap, fmt::Debug};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
struct FalseCountValues<T: Clone + Debug + Serialize + DeserializeOwned> {
    /// The true events which are associated uniquely with a single detections.
    true_positives: T,
    /// The true events which are associated non-uniquely with a single detections.
    ambiguous_true_positives: T,
    /// Detections which are not associated with any true event.
    false_positives: T,
    /// True events which are not associated with any detection.
    false_negatives: T,
}

impl<'a> FromIterator<&'a FalseCountValues<MeanSD>> for Option<FalseCountValues<MeanSD>> {
    fn from_iter<A: IntoIterator<Item = &'a FalseCountValues<MeanSD>>>(iter: A) -> Self {
        let (mut false_counts, len) = iter.into_iter().fold(
            (FalseCountValues::<MeanSD>::default(), 0),
            |(mut sum, len), value| {
                sum.ambiguous_true_positives.mean += value.ambiguous_true_positives.mean;
                sum.ambiguous_true_positives.sd += value.ambiguous_true_positives.sd;
                sum.true_positives.mean += value.true_positives.mean;
                sum.true_positives.sd += value.true_positives.sd;
                sum.false_negatives.mean += value.false_negatives.mean;
                sum.false_negatives.sd += value.false_negatives.sd;
                sum.false_positives.mean += value.false_positives.mean;
                sum.false_positives.sd += value.false_positives.sd;
                (sum, len + 1)
            },
        );
        // Normalise the means, if len != 0, otherwise return `None`.
        if len != 0 {
            false_counts.ambiguous_true_positives.mean /= len as f64;
            false_counts.true_positives.mean /= len as f64;
            false_counts.false_negatives.mean /= len as f64;
            false_counts.false_positives.mean /= len as f64;
            Some(false_counts)
        } else {
            None
        }
    }
}

impl From<&FalseCountValues<SumWithSumOfSqrs>> for FalseCountValues<MeanSD> {
    fn from(value: &FalseCountValues<SumWithSumOfSqrs>) -> FalseCountValues<MeanSD> {
        FalseCountValues {
            true_positives: value.true_positives.mean_and_stddev(),
            ambiguous_true_positives: value.ambiguous_true_positives.mean_and_stddev(),
            false_positives: value.false_positives.mean_and_stddev(),
            false_negatives: value.false_negatives.mean_and_stddev(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PartialFalseCount {
    num: usize,
    true_topic: usize,
    estimate_topic: usize,
    channels: HashMap<Channel, FalseCountValues<SumWithSumOfSqrs>>,
}

impl PartialMetricResultBucket for PartialFalseCount {
    type Source = FlatMetricFalseCount;
    type Complete = CompletedFalseCount;

    fn make_default(source: &FlatMetricFalseCount) -> Self {
        Self {
            num: Default::default(),
            true_topic: source.true_topic,
            estimate_topic: source.estimate_topic,
            channels: Default::default(),
        }
    }

    fn push(
        &mut self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        channel: Channel,
        by_topic: &ChannelDataByTopic,
    ) {
        // true_by_estimates is indexed by the detected events, and the corresponding element is the list of true events that have been associated to it
        // rejected_true is the list of all true events that are not associated with any detected events.
        let (true_by_estimates, rejected_true) =
            self.sort_true_by_estimates(waveform, algorithm, by_topic);

        let ambiguous_true_positives: usize = true_by_estimates
            .iter()
            .filter(|vec| vec.len() > 1)
            .map(|vec| vec.len() - 1)
            .sum();
        let true_positives = true_by_estimates
            .iter()
            .filter(|vec| vec.len() == 1)
            .count();
        let false_positives = true_by_estimates.into_iter().filter(Vec::is_empty).count();
        let false_negatives = rejected_true.len();

        self.num += 1;
        let channel = self.channels.entry(channel).or_default();
        channel
            .ambiguous_true_positives
            .add_to(ambiguous_true_positives as f64);
        channel.true_positives.add_to(true_positives as f64);
        channel.false_positives.add_to(false_positives as f64);
        channel.false_negatives.add_to(false_negatives as f64);
    }
}

impl PartialFalseCount {
    pub(crate) fn sort_true_by_estimates(
        &self,
        waveform: &FlatWaveform,
        _algorithm: &FlatAlgorithm,
        collection_by_topic: &[ChannelData],
    ) -> (Vec<Vec<usize>>, Vec<usize>) {
        let true_data = collection_by_topic
            .get(self.true_topic)
            .expect("Topic should exist, this should never fail.");
        let estimate_data = collection_by_topic
            .get(self.estimate_topic)
            .expect("Topic should exist, this should never fail.");
        let radius = waveform.effective_radius_at_base() as u32;

        let filter = |true_data: &ChannelData, index, detected_time, _detected_intensity| {
            let dist = true_data.get_temporal_distance_from(index, detected_time);
            dist <= radius
        };
        let mut group_data_by = GroupDataBy::new(filter, estimate_data, true_data);
        group_data_by.run();
        group_data_by.finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CompletedFalseCount {
    /// Statistics for each channel.
    channels: HashMap<Channel, FalseCountValues<MeanSD>>,
    /// Statistics for the whole metric
    total: FalseCountValues<MeanSD>,
}

impl CompleteMetricResultBucket for CompletedFalseCount {
    type Partial = PartialFalseCount;
    type Error = MetricResultError;
    type Property = FalseCountProperty;

    fn aggregate(source: &Self::Partial) -> Result<Self, MetricResultError> {
        let channels = source
            .channels
            .iter()
            .map(|(&c, channel)| (c, channel.into()))
            .collect::<HashMap<_, _>>();
        let total = channels
            .values()
            .collect::<Option<_>>()
            .expect("Channels should be non-empty, this should never fail.");
        Ok(Self { channels, total })
    }

    fn get_property(&self, property: FalseCountProperty) -> Result<MetricOutput, Self::Error> {
        match property {
            FalseCountProperty::TotalFalsePositivesMean => {
                Ok(MetricOutput::Value(Some(self.total.false_positives.mean)))
            }
            FalseCountProperty::TotalFalsePositivesSd => Ok(MetricOutput::WithErrors(Some((
                self.total.false_positives.mean,
                self.total.false_positives.sd,
            )))),
            FalseCountProperty::TotalFalseNegativesMean => {
                Ok(MetricOutput::Value(Some(self.total.false_negatives.mean)))
            }
            FalseCountProperty::TotalFalseNegativesSd => Ok(MetricOutput::WithErrors(Some((
                self.total.false_negatives.mean,
                self.total.false_negatives.sd,
            )))),
            FalseCountProperty::TotalTruePositivesMean => {
                Ok(MetricOutput::Value(Some(self.total.true_positives.mean)))
            }
            FalseCountProperty::TotalTruePositivesSd => Ok(MetricOutput::WithErrors(Some((
                self.total.true_positives.mean,
                self.total.true_positives.sd,
            )))),
            FalseCountProperty::TotalAmbiguousTruePositivesMean => Ok(MetricOutput::Value(Some(
                self.total.ambiguous_true_positives.mean,
            ))),
            FalseCountProperty::TotalAmbiguousTruePositivesSd => {
                Ok(MetricOutput::WithErrors(Some((
                    self.total.ambiguous_true_positives.mean,
                    self.total.ambiguous_true_positives.sd,
                ))))
            }
            FalseCountProperty::ChannelsAmbiguousTruePositivesBoxPlot => Ok(MetricOutput::Group(
                self.channels
                    .iter()
                    .map(|(channel, stats)| {
                        Some((
                            stats.ambiguous_true_positives.mean,
                            format!("Channel {channel}"),
                        ))
                    })
                    .collect(),
            )),
            FalseCountProperty::ChannelsFalseNegativesBoxPlot => Ok(MetricOutput::Group(
                self.channels
                    .iter()
                    .map(|(channel, stats)| {
                        Some((stats.false_negatives.mean, format!("Channel {channel}")))
                    })
                    .collect(),
            )),
            FalseCountProperty::ChannelsFalsePositivesBoxPlot => Ok(MetricOutput::Group(
                self.channels
                    .iter()
                    .map(|(channel, stats)| {
                        Some((stats.false_positives.mean, format!("Channel {channel}")))
                    })
                    .collect(),
            )),
            FalseCountProperty::ChannelsTruePositivesBoxPlot => Ok(MetricOutput::Group(
                self.channels
                    .iter()
                    .map(|(channel, stats)| {
                        Some((stats.true_positives.mean, format!("Channel {channel}")))
                    })
                    .collect(),
            )),
        }
    }
}
