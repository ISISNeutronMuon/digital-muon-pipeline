use digital_muon_common::Channel;
use std::collections::HashMap;

use crate::{
    analysis::metrics::{
        MetricAggregatedResult, MetricChannelResult, MetricOutput, SumWithSumOfSqrs,
    },
    engine::{FlatAlgorithm, FlatWaveform, MetricProperty},
    event::ChannelData,
};

#[derive(Clone, Debug)]
pub(crate) struct MuonLifetime {
    num: usize,
    lifetime: SumWithSumOfSqrs,
}

impl MetricChannelResult for MuonLifetime {
    type Source = ();
    type Aggregrate = CompletedMuonLifetime;

    fn make_default(_: ()) -> Self {
        Self {
            num: Default::default(),
            lifetime: Default::default(),
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
    lifetime_mean: f64,
    lifetime_sd: f64,
}

impl MetricAggregatedResult for CompletedMuonLifetime {
    type Channel = MuonLifetime;

    fn aggregate(source: &HashMap<Channel, Self::Channel>) -> Self {
        /*let (sum_of_means, sum_of_sds) = source
            .values()
            .map(|count| count.lifetime.mean_and_stddev(count.num as f64))
            .fold(
                Default::default(),
                |(acc_mean, acc_sd): (f64, f64), (mean, sd)| (acc_mean + mean, acc_sd + sd),
            );
        let lifetime_mean = sum_of_means / source.len() as f64;
        let lifetime_sd = sum_of_sds / source.len() as f64;
        */

        let (lifetime_mean, lifetime_sd) = Self::stats_aggregator(source.values(), source.len() as f64,
            |count|count.lifetime.mean_and_stddev(count.num as f64)
        );

        Self {
            lifetime_mean,
            lifetime_sd,
        }
    }

    fn get_property(&self, property: &MetricProperty) -> Result<MetricOutput<f64>, String> {
        match property {
            MetricProperty::Mean => Ok(MetricOutput::Scalar(self.lifetime_mean)),
            MetricProperty::SD => Ok(MetricOutput::ScalarWithBand(
                self.lifetime_mean,
                self.lifetime_sd,
            )),
            _ => unreachable!(),
        }
    }
}
