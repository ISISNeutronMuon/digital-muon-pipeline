use serde::{Deserialize, Serialize};
use crate::{
    analysis::metrics::{
        CompleteMetricResultClass, PartialMetricResultClass, MetricOutput, SumWithSumOfSqrs,
    },
    engine::{FlatAlgorithm, FlatWaveform, MetricProperty},
    event::ChannelData,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MuonLifetime {
    num: usize,
    lifetime: SumWithSumOfSqrs,
}

impl PartialMetricResultClass for MuonLifetime {
    type Source = ();
    type Complete = CompletedMuonLifetime;

    fn make_default(_: &()) -> Self {
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

    fn len(&self) -> usize {
        self.num
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CompletedMuonLifetime {
    lifetime_mean: f64,
    lifetime_sd: f64,
}

impl CompleteMetricResultClass for CompletedMuonLifetime {
    type Partial = MuonLifetime;

    fn aggregate(source: &Self::Partial) -> Self {
        let (lifetime_mean, lifetime_sd) = source.lifetime.mean_and_stddev(source.num as f64);
            /*Self::stats_aggregator(source.values(), source.len() as f64, |count| {
                count.lifetime.mean_and_stddev(count.num as f64)
            });*/

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
