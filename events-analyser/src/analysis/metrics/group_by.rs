use crate::event::ChannelData;
use digital_muon_common::{Intensity, Time};
use std::iter::once;

pub(super) struct GroupDataBy<'a, F>
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
    pub(super) fn new(
        data_filter: F,
        group_labels: &'a ChannelData,
        data_domain: &'a ChannelData,
    ) -> Self {
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

    pub(super) fn filter(
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
                .expect("data_bucket should have at least `group_index` elements, this should never fail.")
                .push(domain_index);
        } else {
            self.reject_bucket.push(domain_index);
        }
    }

    pub(super) fn is_group_label_at_index_less_than_current_domain_time(
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

    pub(super) fn run(&mut self) {
        // Iterator which iterates through `[None, Some(0), Some(1), ..., Some(some.num_groups - 1)]`.
        // `None` indicates no left-bound is present, `Some(i)` indicates the left-bound is the ith
        // element of `data_domain`.
        let mut labels_left_bound = once(None)
            .chain((0..(self.num_groups - 1)).map(Some))
            .peekable();

        for (domain_index, (domain_time, domain_intensity)) in
            self.data_domain.get_time_intensity().iter().enumerate()
        {
            loop {
                if let Some(labels_left_bound_index) = labels_left_bound.peek() {
                    match labels_left_bound_index {
                        None => {
                            // If the first `data_domain` item is less than the current `group_labels` item.
                            if self.is_group_label_at_index_less_than_current_domain_time(
                                0,
                                *domain_time,
                            ) {
                                labels_left_bound.next();
                            } else {
                                break;
                            }
                        }
                        Some(labels_left_bound_index) => {
                            // If the next `data_domain` item is less than the current `group_labels` item.
                            if self.is_group_label_at_index_less_than_current_domain_time(
                                labels_left_bound_index + 1,
                                *domain_time,
                            ) {
                                labels_left_bound.next();
                            } else {
                                break;
                            }
                        }
                    }
                } else {
                    break;
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

    pub(super) fn finish(self) -> (Vec<Vec<usize>>, Vec<usize>) {
        (self.data_bucket, self.reject_bucket)
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

    #[test]
    fn trivial_test() {
        let group_labels = ChannelData::new((0..100).map(|i| (i * 15, 10)).collect());
        let data_domain = ChannelData::new((0..100).map(|i| (i * 15, 10)).collect());

        let data_filter = |_, _, _, _| true;
        let mut group_data_by = GroupDataBy::new(data_filter, &group_labels, &data_domain);
        group_data_by.run();
        let (grouped_data, reject_data) = group_data_by.finish();

        assert!(reject_data.is_empty());
        assert_eq!(grouped_data.len(), 100);
        for (i, v) in grouped_data.into_iter().enumerate() {
            assert_eq!(v.len(), 1);
            assert_eq!(i, v[0]);
        }
    }
    /*
    #[test]
    fn nontrivial_test() {
        let group_labels = ChannelData::new((0..100).map(|i|(i*15, 10)).collect());
        let data_domain = ChannelData::new((0..100).map(|i|(i*15 + (10.0*f64::sin(8.0*3.141592 * i as f64/100.0)) as u32, 10)).collect());

        let data_filter = |_, _, _, _| true;
        let mut group_data_by = GroupDataBy::new(data_filter, &group_labels, &data_domain);
        group_data_by.run();
        let (grouped_data, reject_data) = group_data_by.finish();

        println!("{grouped_data:?}");
        assert!(reject_data.is_empty());
        assert_eq!(grouped_data.len(), 100);
        for (i,v) in grouped_data.into_iter().enumerate() {
            assert_eq!(v.len(), 1);
            assert_eq!(i, v[0]);
        }
    } */
}
