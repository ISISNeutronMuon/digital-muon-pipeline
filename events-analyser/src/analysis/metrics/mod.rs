mod false_counts;
mod partial_results;

use std::collections::HashMap;
use digital_muon_common::Channel;
use crate::{
    engine::{FlatAlgorithm, FlatWaveform},
    event::ChannelData
};

pub(crate) use partial_results::PatrtialMetricResult;

#[derive(Default, Clone)]
struct SumWithSumOfSqrs {
    sum: f64,
    sqr_sum: f64,
}

impl SumWithSumOfSqrs {
    fn add_to(&mut self, value: f64) {
        self.sum += value;
        self.sqr_sum += value * value;
    }

    pub(crate) fn mean_and_stddev(&self, n: f64) -> (f64, f64) {
        (
            self.sum / n,
            f64::sqrt((n * self.sqr_sum - self.sum * self.sum) / (n * (n - 1.0))),
        )
    }
}

trait MetricChannelResult : Clone {
    type Source;
    type Aggregrate: MetricAggregatedResult<Channel = Self>;
    
    fn make_default(source: Self::Source) -> Self;
    fn push(&mut self, waveform: &FlatWaveform, algorithm: &FlatAlgorithm, by_topic : &[ChannelData]);
}

trait MetricAggregatedResult : Clone {
    type Channel: MetricChannelResult<Aggregrate = Self>;

    fn aggregate(source: &HashMap<Channel, Self::Channel>) -> Self;
}