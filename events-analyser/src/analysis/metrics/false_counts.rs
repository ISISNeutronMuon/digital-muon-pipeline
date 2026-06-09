use crate::{
    analysis::metrics::{
        CompleteMetricResultClass, MetricOutput, PartialMetricResultClass, SumWithSumOfSqrs,
        group_by::GroupDataBy,
    },
    engine::{FlatAlgorithm, FlatMetricFalseCount, FlatWaveform, MetricProperty},
    event::ChannelData,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct FalseCount {
    num: usize,
    true_topic: usize,
    estimate_topic: usize,
    positive_sum: SumWithSumOfSqrs,
    negative_sum: SumWithSumOfSqrs,
}

impl PartialMetricResultClass for FalseCount {
    type Source = FlatMetricFalseCount;
    type Complete = CompletedFalseCount;

    fn make_default(source: &FlatMetricFalseCount) -> Self {
        Self {
            num: Default::default(),
            true_topic: source.true_topic,
            estimate_topic: source.estimate_topic,
            positive_sum: Default::default(),
            negative_sum: Default::default(),
        }
    }

    fn push(
        &mut self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        by_topic: &[ChannelData],
    ) {
        self.num += 1;
        let (positives, negatives) = self.get_false_counts(waveform, algorithm, by_topic);
        self.positive_sum.add_to(positives as f64);
        self.negative_sum.add_to(negatives as f64);
    }

    fn len(&self) -> usize {
        self.num
    }
}

impl FalseCount {
    /*pub(crate) fn sort_estimates_by_true(
        &self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        collection_by_topic: &[ChannelData],
    ) -> (Vec<Vec<usize>>, Vec<usize>) {
        let true_data = collection_by_topic
            .get(self.true_topic)
            .expect("Topic should exist, this should never fail.");
        let estimate_data = collection_by_topic
            .get(self.estimate_topic)
            .expect("Topic should exist, this should never fail.");

        let filter = |data_to_group: &ChannelData, index, time, intensity| {
            //let dist = data_to_group.get_time_intensity().get_temporal_distance_from(index, time);
            let detected = data_to_group.get_time_intensity_of_index(index);
            algorithm.is_true_positive(waveform, (time, intensity), detected)
        };
        let mut group_data_by = GroupDataBy::new(filter, true_data, estimate_data);
        group_data_by.run();
        group_data_by.finish()
    }*/

    pub(crate) fn sort_true_by_estimates(
        &self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
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

    pub(crate) fn get_false_counts(
        &self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        collection_by_topic: &[ChannelData],
    ) -> (usize, usize) {
        let (true_by_estimates, estimate_reject) =
            self.sort_true_by_estimates(waveform, algorithm, collection_by_topic);
        let false_positives = true_by_estimates.into_iter().filter(Vec::is_empty).count();
        let false_negatives = estimate_reject.len();

        (false_positives, false_negatives)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CompletedFalseCount {
    positive_mean: f64,
    positive_sd: f64,
    negative_mean: f64,
    negative_sd: f64,
}

impl CompleteMetricResultClass for CompletedFalseCount {
    type Partial = FalseCount;

    fn aggregate(source: &Self::Partial) -> Self {
        let (positive_mean, positive_sd) = source.positive_sum.mean_and_stddev(source.num as f64);
        /*Self::stats_aggregator(source.values(), source.len() as f64, |count| {
            count.positive_sum.mean_and_stddev(count.num as f64)
        });*/
        let (negative_mean, negative_sd) = source.negative_sum.mean_and_stddev(source.num as f64);
        /*let (negative_mean, negative_sd) =
        Self::stats_aggregator(source.values(), source.len() as f64, |count| {
            count.negative_sum.mean_and_stddev(count.num as f64)
        });*/
        Self {
            positive_mean,
            positive_sd,
            negative_mean,
            negative_sd,
        }
    }

    fn get_property(&self, property: &MetricProperty) -> Result<MetricOutput<f64>, String> {
        match property {
            MetricProperty::FalsePositivesMean => Ok(MetricOutput::Scalar(self.positive_mean)),
            MetricProperty::FalsePositivesSD => Ok(MetricOutput::ScalarWithBand(
                self.positive_mean,
                self.positive_sd,
            )),
            MetricProperty::FalseNegativesMean => Ok(MetricOutput::Scalar(self.negative_mean)),
            MetricProperty::FalseNegativesSD => Ok(MetricOutput::ScalarWithBand(
                self.negative_mean,
                self.negative_sd,
            )),
            _ => unreachable!(),
        }
    }
}
