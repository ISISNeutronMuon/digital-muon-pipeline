use digital_muon_common::{Channel, Intensity, Time};
use std::{collections::HashMap, iter::once};

use crate::{
    analysis::metrics::{
        MetricAggregatedResult, MetricChannelResult, MetricOutput, SumWithSumOfSqrs,
    },
    engine::{FlatAlgorithm, FlatMetricFalseCount, FlatWaveform},
    event::ChannelData,
};

#[derive(Clone, Debug)]
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
        let (positives, negatives) = self.get_false_counts(waveform, algorithm, by_topic);
        self.positive_sum.add_to(positives as f64);
        self.negative_sum.add_to(negatives as f64);
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

struct GroupDataBy<'a, F>
where
    F: Fn(&'a ChannelData, usize, Time, Intensity) -> bool,
{
    data_filter: F,
    group_labels: &'a ChannelData,
    data_domain: &'a ChannelData,
    num_groups: usize,
    data_bucket: Vec<Vec<usize>>,
    reject_bucket: Vec<usize>,
}

impl<'a, F> GroupDataBy<'a, F>
where
    F: Fn(&'a ChannelData, usize, Time, Intensity) -> bool,
{
    fn new(data_filter: F, group_labels: &'a ChannelData, data_domain: &'a ChannelData) -> Self {
        let num_groups = group_labels.get_time_intensity().len();

        let data_bucket = vec![Vec::<usize>::new(); num_groups];
        let reject_bucket = Vec::<usize>::new();
        Self {
            data_filter,
            group_labels,
            data_domain,
            num_groups,
            data_bucket,
            reject_bucket,
        }
    }

    fn filter(
        &mut self,
        group_index: usize,
        domain_index: usize,
        domain_time: Time,
        domain_intensity: Intensity,
    ) {
        if (&self.data_filter)(
            self.group_labels,
            group_index,
            domain_time,
            domain_intensity,
        ) {
            self.data_bucket.get_mut(group_index)
                .expect("data_bucket should have at least `domain_index` elements, this should never fail.")
                .push(domain_index);
        } else {
            self.reject_bucket.push(domain_index);
        }
    }

    fn is_group_label_at_index_less_than_current_domain_time(
        &self,
        group_label_index: usize,
        current_domain_time: Time,
    ) -> bool {
        self.group_labels
            .get_time_intensity()
            .get(group_label_index)
            .expect("This should never fail.")
            .0
            < current_domain_time
    }

    fn run(&mut self) {
        // Iterator which iterates through `[None, Some(0), Some(1), ..., Some(some.num_data_points - 1)]`.
        // `None` indicates no left-bound is present, `Some(i)` indicates the left-bound is the ith
        // element of `data_domain`.
        let mut labels_left_bound = once(None)
            .chain((0..(self.num_groups - 1)).map(Some))
            .peekable();

        for (domain_index, (domain_time, domain_intensity)) in
            self.data_domain.get_time_intensity().iter().enumerate()
        {
            if let Some(labels_left_bound_index) = labels_left_bound.peek() {
                match labels_left_bound_index {
                    None => {
                        // If the first `data_domain` item is less than the current `group_labels` item.
                        if self
                            .is_group_label_at_index_less_than_current_domain_time(0, *domain_time)
                        {
                            labels_left_bound.next();
                        }
                    }
                    Some(labels_left_bound_index) => {
                        // If the next `data_domain` item is less than the current `group_labels` item.
                        if self.is_group_label_at_index_less_than_current_domain_time(
                            labels_left_bound_index + 1,
                            *domain_time,
                        ) {
                            labels_left_bound.next();
                        }
                    }
                }
            }
            match labels_left_bound.peek() {
                // When `labels_left_bound` is left of the first `group_label` item.
                Some(None) => {
                    self.filter(0, domain_index, *domain_time, *domain_intensity);
                }
                // When `labels_left_bound` is between the first and last `group_label` items.
                Some(Some(labels_left_bound_index)) => {
                    let nearest_bucket_index = self
                        .group_labels
                        .find_nearest_in_time_after_index(*labels_left_bound_index, *domain_time);

                    self.filter(
                        nearest_bucket_index,
                        domain_index,
                        *domain_time,
                        *domain_intensity,
                    );
                }
                // When `labels_left_bound` is right of the last `group_label` item.
                None => {
                    self.filter(
                        self.num_groups - 1,
                        domain_index,
                        *domain_time,
                        *domain_intensity,
                    );
                }
            }
        }
    }

    fn finish(self) -> (Vec<Vec<usize>>, Vec<usize>) {
        (self.data_bucket, self.reject_bucket)
    }
}

impl FalseCount {
    pub(crate) fn sort_estimates_by_true(
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
            let dist = data_to_group.get_temporal_distance_from(index, time);
            algorithm.is_true_positive(waveform, time, intensity, dist)
        };
        let mut group_data_by = GroupDataBy::new(filter, true_data, estimate_data);
        group_data_by.run();
        group_data_by.finish()
    }

    pub(crate) fn get_false_counts(
        &self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        collection_by_topic: &[ChannelData],
    ) -> (usize, usize) {
        let (true_data_bucket, true_data_reject) =
            self.sort_estimates_by_true(waveform, algorithm, collection_by_topic);
        let false_positives = true_data_reject.len();
        let false_negatives = true_data_bucket.into_iter().filter(Vec::is_empty).count();
        (false_positives, false_negatives)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        let group_labels = ChannelData::new(vec![(31, 6), (50, 12)]);
        let data_domain = ChannelData::new(vec![(40, 6), (60, 12)]);

        let mut group_data_by = GroupDataBy::new(|_, _, _, _| true, &group_labels, &data_domain);
        group_data_by.run();
        let (grouped_data, reject_data) = group_data_by.finish();

        assert!(reject_data.is_empty());
        assert_eq!(grouped_data, vec![vec![0], vec![1]]);
    }

    #[test]
    fn test2() {
        let group_labels = ChannelData::new(vec![(30, 6), (50, 12)]);
        let data_domain = ChannelData::new(vec![(40, 6), (60, 12)]);

        let mut group_data_by = GroupDataBy::new(|_, _, _, _| true, &group_labels, &data_domain);
        group_data_by.run();
        let (grouped_data, reject_data) = group_data_by.finish();

        assert!(reject_data.is_empty());
        assert_eq!(grouped_data, vec![vec![], vec![0, 1]]);
    }

    #[test]
    fn test3() {
        let group_labels = ChannelData::new(vec![(49, 6), (77, 12)]);
        let data_domain = ChannelData::new(vec![(40, 6), (60, 12)]);

        let mut group_data_by = GroupDataBy::new(|_, _, _, _| true, &group_labels, &data_domain);
        group_data_by.run();
        let (grouped_data, reject_data) = group_data_by.finish();

        assert!(reject_data.is_empty());
        assert_eq!(grouped_data, vec![vec![0, 1], vec![]]);
    }

    #[test]
    fn test4() {
        let group_labels = ChannelData::new(vec![(49, 6), (55, 6), (77, 12)]);
        let data_domain = ChannelData::new(vec![
            (40, 6),
            (54, 6),
            (60, 12),
            (61, 12),
            (62, 12),
            (76, 12),
            (79, 12),
        ]);

        const WIDTH: Time = 4;
        let data_filter = |group_labels: &ChannelData, group_index, time, _| {
            group_labels.get_temporal_distance_from(group_index, time) <= WIDTH
        };
        let mut group_data_by = GroupDataBy::new(data_filter, &group_labels, &data_domain);
        group_data_by.run();
        let (grouped_data, reject_data) = group_data_by.finish();

        assert_eq!(grouped_data, vec![vec![], vec![1], vec![5, 6]]);
        assert_eq!(reject_data, vec![0, 2, 3, 4]);
    }
}
