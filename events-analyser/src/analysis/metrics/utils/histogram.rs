use crate::engine::Interval;
use serde::{Deserialize, Serialize};
use std::ops::AddAssign;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Histogram {
    num_values: usize,
    bins: Vec<f64>,
    bin_labels: Vec<f64>,
    interval: Interval<f64>,
    num_too_small: usize,
    num_too_big: usize,
}

impl Histogram {
    pub(crate) fn new(num: usize, interval: &Interval<f64>) -> Self {
        let bins = vec![Default::default(); num];
        let coef = (interval.max - interval.min) / num as f64;
        let bin_labels = (0..num).map(|i| i as f64 * coef + interval.min).collect();
        Self {
            num_values: Default::default(),
            bin_labels,
            bins,
            interval: interval.clone(),
            num_too_small: Default::default(),
            num_too_big: Default::default(),
        }
    }

    pub(crate) fn push(&mut self, value: f64) {
        if self.interval.min <= value && value < self.interval.max {
            self.num_values += 1;
            let index = (self.bins.len() as f64 * (value - self.interval.min)
                / (self.interval.max - self.interval.min)) as usize;
            self.bins
                .get_mut(index)
                .expect("Element should exist, this should never fail")
                .add_assign(1.0);
        } else if value < self.interval.min {
            self.num_too_small += 1;
        } else {
            self.num_too_big += 1;
        }
    }

    pub(crate) fn get_bin_labels(&self) -> &[f64] {
        &self.bin_labels
    }

    pub(crate) fn get_counts(&self) -> &[f64] {
        &self.bins
    }

    pub(crate) fn get_normalised_counts(&self) -> Vec<f64> {
        let coef = 1.0 / self.num_values as f64;
        self.bins.iter().map(|value| value * coef).collect()
    }

    #[cfg(test)]
    pub(crate) fn set(&mut self, bins: Vec<f64>) {
        self.num_values = bins.iter().sum::<f64>() as usize;
        self.bins = bins;
    }
}
