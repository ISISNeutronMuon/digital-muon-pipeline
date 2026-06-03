mod event_counts;
mod false_counts;
mod output;
mod muon_lifetime;
mod result;

use crate::{
    engine::{FlatAlgorithm, FlatWaveform, MetricProperty},
    event::ChannelData,
};
use digital_muon_common::Channel;
use std::collections::HashMap;

pub(crate) use result::MetricResult;
pub(crate) use output::MetricOutput;

#[derive(Default, Debug, Clone)]
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

pub(crate) trait MetricChannelResult : Clone {
    type Source;
    type Aggregrate: MetricAggregatedResult<Channel = Self>;

    fn make_default(source: Self::Source) -> Self;
    fn push(
        &mut self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        by_topic: &[ChannelData],
    );
}

trait MetricAggregatedResult: Clone {
    type Channel: MetricChannelResult<Aggregrate = Self>;

    fn stats_aggregator<'a, F, I>(source: I, len: f64, f : F) -> (f64, f64)
        where
            F : Fn(&'a Self::Channel) -> (f64,f64), Self::Channel : 'a,
            I : Iterator<Item = &'a Self::Channel> + ExactSizeIterator
    {
        let (sum_of_means, sum_of_sds) = source
            .map(|count| f(count))
            .fold(
                Default::default(),
                |(acc_mean, acc_sd): (f64, f64), (mean, sd)| (acc_mean + mean, acc_sd + sd),
            );
        let mean = sum_of_means / len;
        let sd = sum_of_sds / len;
        (mean, sd)
    }
    fn aggregate(source: &HashMap<Channel, Self::Channel>) -> Self;
    fn get_property(&self, property: &MetricProperty) -> Result<MetricOutput<f64>, String>;
}
