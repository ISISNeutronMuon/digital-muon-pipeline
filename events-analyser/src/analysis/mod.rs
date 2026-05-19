//! Defines the data type used in [FrameCache].
//!
//! [FrameCache]: crate::frame::FrameCache

use digital_muon_common::Channel;

struct SumWithSumOfSqrs {
    sum: f64,
    sqr_sum: f64
}

impl SumWithSumOfSqrs {
    fn add_to(&mut self, value: f64) {
        self.sum += value;
        self.sqr_sum += value*value;
    }

    fn mean_and_stddev(&self, n: f64) -> (f64, f64) {
        (self.sum/n, f64::sqrt((n*self.sqr_sum - self.sum*self.sum)/(n*(n - 1.0))))
    }
}

pub(crate) struct PartialChannelAnalysis {
    num_frames: usize,
    num_false_positives: SumWithSumOfSqrs,
    num_false_negatives: SumWithSumOfSqrs,
}

pub(crate) struct Analysis {
    channel: Vec<PartialChannelAnalysis>,
}
