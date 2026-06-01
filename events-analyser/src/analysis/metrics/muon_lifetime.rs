use digital_muon_common::Channel;
use std::collections::HashMap;

use crate::{
    analysis::metrics::{
        MetricAggregatedResult, MetricChannelResult, MetricOutput, SumWithSumOfSqrs,
    },
    engine::{FlatAlgorithm, FlatWaveform},
    event::ChannelData,
};

#[derive(Clone, Debug)]
pub(crate) struct MuonLifetime {
    num: usize,
    positive_sum: SumWithSumOfSqrs,
    negative_sum: SumWithSumOfSqrs,
}

impl MetricChannelResult for MuonLifetime {
    type Source = ();
    type Aggregrate = CompletedMuonLifetime;

    fn make_default(_: ()) -> Self {
        Self {
            num: Default::default(),
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
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CompletedMuonLifetime {
    positive_mean: f64,
    positive_sd: f64,
    negative_mean: f64,
    negative_sd: f64,
}

impl MetricAggregatedResult for CompletedMuonLifetime {
    type Channel = MuonLifetime;

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
