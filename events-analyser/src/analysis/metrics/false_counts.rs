use crate::{
    analysis::metrics::{
        CompleteMetricResultClass, MeanSD, MetricOutput, PartialMetricResultClass,
        SumWithSumOfSqrs, utils::GroupDataBy,
    },
    engine::{FlatAlgorithm, FlatMetricFalseCount, FlatWaveform, MetricProperty},
    event::ChannelData,
    eventlists::ChannelDataByTopic,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct FalseCount {
    num: usize,
    true_topic: usize,
    estimate_topic: usize,
    /// The true events which are associated uniquely with a single detections.
    true_positive_sum: SumWithSumOfSqrs,
    /// The true events which are associated non-uniquely with a single detections.
    ambiguous_true_positive_sum: SumWithSumOfSqrs,
    /// Detections which are not associated with any true event.
    false_positive_sum: SumWithSumOfSqrs,
    /// True events which are not associated with any detection.
    false_negative_sum: SumWithSumOfSqrs,
}

impl PartialMetricResultClass for FalseCount {
    type Source = FlatMetricFalseCount;
    type Complete = CompletedFalseCount;

    fn make_default(source: &FlatMetricFalseCount) -> Self {
        Self {
            num: Default::default(),
            true_topic: source.true_topic,
            estimate_topic: source.estimate_topic,
            true_positive_sum: Default::default(),
            ambiguous_true_positive_sum: Default::default(),
            false_positive_sum: Default::default(),
            false_negative_sum: Default::default(),
        }
    }

    fn push(
        &mut self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
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
        self.ambiguous_true_positive_sum
            .add_to(ambiguous_true_positives as f64);
        self.true_positive_sum.add_to(true_positives as f64);
        self.false_positive_sum.add_to(false_positives as f64);
        self.false_negative_sum.add_to(false_negatives as f64);
    }

    fn len(&self) -> usize {
        self.num
    }
}

impl FalseCount {
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
    /// The true events which are associated uniquely with a single detections.
    true_positives: MeanSD,
    /// The true events which are associated non-uniquely with a single detections.
    ambiguous_true_positives: MeanSD,
    /// Detections which are not associated with any true event.
    false_positives: MeanSD,
    /// True events which are not associated with any detection.
    false_negatives: MeanSD,
}

impl CompleteMetricResultClass for CompletedFalseCount {
    type Partial = FalseCount;
    type Error = ();

    fn aggregate(source: &Self::Partial) -> Result<Self, ()> {
        Ok(Self {
            true_positives: source.true_positive_sum.mean_and_stddev(),
            ambiguous_true_positives: source.ambiguous_true_positive_sum.mean_and_stddev(),
            false_positives: source.false_positive_sum.mean_and_stddev(),
            false_negatives: source.false_negative_sum.mean_and_stddev(),
        })
    }

    fn get_property(&self, property: &MetricProperty) -> Result<MetricOutput<f64>, String> {
        match property {
            MetricProperty::FalsePositivesMean => {
                Ok(MetricOutput::Scalar(self.false_positives.mean))
            }
            MetricProperty::FalsePositivesSD => Ok(MetricOutput::ScalarWithBand(
                self.false_positives.mean,
                self.false_positives.sd,
            )),
            MetricProperty::FalseNegativesMean => {
                Ok(MetricOutput::Scalar(self.false_negatives.mean))
            }
            MetricProperty::FalseNegativesSD => Ok(MetricOutput::ScalarWithBand(
                self.false_negatives.mean,
                self.false_negatives.sd,
            )),
            MetricProperty::TruePositivesMean => Ok(MetricOutput::Scalar(self.true_positives.mean)),
            MetricProperty::TruePositivesSD => Ok(MetricOutput::ScalarWithBand(
                self.true_positives.mean,
                self.true_positives.sd,
            )),
            MetricProperty::AmbiguousTruePositivesMean => {
                Ok(MetricOutput::Scalar(self.ambiguous_true_positives.mean))
            }
            MetricProperty::AmbiguousTruePositivesSD => Ok(MetricOutput::ScalarWithBand(
                self.ambiguous_true_positives.mean,
                self.ambiguous_true_positives.sd,
            )),
            _ => unreachable!(),
        }
    }
}
