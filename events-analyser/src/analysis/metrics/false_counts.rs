use digital_muon_common::{Channel, Intensity, Time};
use std::{collections::HashMap, iter::once};

use crate::{
    analysis::metrics::{
        MetricAggregatedResult, MetricChannelResult, MetricOutput, SumWithSumOfSqrs,
    },
    engine::{FlatAlgorithm, FlatMetricFalseCount, FlatWaveform, Metric, WithName},
    event::ChannelData,
};

#[derive(Clone,Debug)]
pub(crate) struct FalseCount {
    num: usize,
    true_topic: usize,
    estimate_topic: usize,
    positive_sum: SumWithSumOfSqrs,
    negative_sum: SumWithSumOfSqrs,
}

impl MetricChannelResult for FalseCount {
    type Source = FlatMetricFalseCount;
    type Aggregrate = CompletedFalseCount;

    fn make_default(source: FlatMetricFalseCount) -> Self {
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
        self.positive_sum.add_to(get_false_positives(
            self.true_topic,
            self.estimate_topic,
            waveform,
            algorithm,
            by_topic,
        ));
        self.negative_sum.add_to(get_false_positives(
            self.true_topic,
            self.estimate_topic,
            waveform,
            algorithm,
            by_topic,
        ));
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CompletedFalseCount {
    positive_mean: f64,
    positive_sd: f64,
    negative_mean: f64,
    negative_sd: f64,
}

impl MetricAggregatedResult for CompletedFalseCount {
    type Channel = FalseCount;

    fn aggregate(source: &HashMap<Channel, Self::Channel>) -> Self {
        let (sum_of_means, sum_of_sds) = source
            .values()
            .map(|count| count.positive_sum.mean_and_stddev(count.num as f64))
            .fold(
                Default::default(),
                |(acc_mean, acc_sd): (f64, f64), (mean, sd)| (acc_mean + mean, acc_sd + sd),
            );
        let positive_mean = sum_of_means / source.len() as f64;
        let positive_sd = sum_of_sds / source.len() as f64;

        let (sum_of_means, sum_of_sds) = source
            .values()
            .map(|count| count.negative_sum.mean_and_stddev(count.num as f64))
            .fold(
                Default::default(),
                |(acc_mean, acc_sd): (f64, f64), (mean, sd)| (acc_mean + mean, acc_sd + sd),
            );
        let negative_mean = sum_of_means / source.len() as f64;
        let negative_sd = sum_of_sds / source.len() as f64;
        Self {
            positive_mean,
            positive_sd,
            negative_mean,
            negative_sd,
        }
    }

    fn get_property(&self, property: &str) -> Result<MetricOutput<f64>, String> {
        match property {
            "false-positives-mean" => Ok(MetricOutput::Scalar(self.positive_mean)),
            "false-positives-sd" => Ok(MetricOutput::ScalarWithBand(
                self.positive_mean,
                self.positive_sd,
            )),
            "false-negatives-mean" => Ok(MetricOutput::Scalar(self.negative_mean)),
            "false-negatives-sd" => Ok(MetricOutput::ScalarWithBand(
                self.negative_mean,
                self.negative_sd,
            )),
            _ => Err(format!("No property matching {property}")),
        }
    }
}

fn group_data_by<F>(
    filter: F,
    data_to_group_by: &ChannelData,
    data_to_group: &ChannelData,
) -> (Vec<Vec<usize>>, Vec<usize>)
where
    F: Fn(&ChannelData, usize, Time, Intensity) -> bool,
{
    let num_groups = data_to_group_by.get_time_intensity().len();
    let num_data_points = data_to_group.get_time_intensity().len();

    // Iterator which iterates through `[None, Some(0), Some(1), ..., Some(estimate_len - 2)]`.
    // `None` indicates no left-bound is present, `Some(i)` indicates the left bound is the ith
    // element of `data_to_group`.
    let mut left_bound = once(None)
        .chain((0..(num_data_points - 1)).map(Some))
        .peekable();

    let mut data_bucket = vec![Vec::<usize>::new(); num_groups];
    let mut reject_bucket = Vec::<usize>::new();
    for (index, (time, intensity)) in data_to_group_by.get_time_intensity().iter().enumerate() {
        match left_bound.peek() {
            None => {
                if filter(data_to_group, 0, *time, *intensity) {
                    data_bucket
                        .first_mut()
                        .expect("data_bucket should be non-empty, this should never fail.")
                        .push(index);
                } else {
                    reject_bucket.push(index);
                }

                if data_to_group.get_time_intensity().first().unwrap().0 < *time {
                    left_bound.next();
                }
            }
            Some(Some(left_bound_index)) => {
                let index =
                    data_to_group.find_nearest_in_time_after_index(*left_bound_index, *time);

                if filter(data_to_group, index, *time, *intensity) {
                    data_bucket.get_mut(index)
                        .expect("data_bucket should have at least index elements, this should never fail.")
                        .push(index);
                } else {
                    reject_bucket.push(index);
                }

                if data_to_group.get_time_intensity()[*left_bound_index + 1].0 < *time {
                    left_bound.next();
                }
            }
            Some(None) => {
                if filter(data_to_group, num_data_points - 1, *time, *intensity) {
                    data_bucket
                        .last_mut()
                        .expect("data_bucket should be non-empty, this should never fail.")
                        .push(index);
                } else {
                    reject_bucket.push(index);
                }
            }
        }
    }
    (data_bucket, reject_bucket)
}

pub(crate) fn sort_estimates_by_true(
    true_topic_index: usize,
    estimate_topic_index: usize,
    waveform: &FlatWaveform,
    algorithm: &FlatAlgorithm,
    collection_by_topic: &[ChannelData],
) -> (Vec<Vec<usize>>, Vec<usize>) {
    let true_data = collection_by_topic
        .get(true_topic_index)
        .expect("Topic should exist, this should never fail.");
    let estimate_data = collection_by_topic
        .get(estimate_topic_index)
        .expect("Topic should exist, this should never fail.");

    let filter = |data_to_group: &ChannelData, index, time, intensity| {
        let dist = data_to_group
            .get_temporal_distance_from(index, time)
            .expect("Length of data should be > index, this should never fail.");
        algorithm.is_true_positive(waveform, time, intensity, dist)
    };
    group_data_by(filter, true_data, estimate_data)
}

pub(crate) fn get_false_positives(
    true_topic_index: usize,
    estimate_topic_index: usize,
    waveform: &FlatWaveform,
    algorithm: &FlatAlgorithm,
    collection_by_topic: &[ChannelData],
) -> f64 {
    let true_data_bucket = sort_estimates_by_true(
        true_topic_index,
        estimate_topic_index,
        waveform,
        algorithm,
        collection_by_topic,
    );
    1.0
}
