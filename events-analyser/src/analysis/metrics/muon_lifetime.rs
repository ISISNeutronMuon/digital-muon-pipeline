use crate::{
    analysis::metrics::{
        CompleteMetricResultClass, MeanSD, MetricOutput, PartialMetricResultClass, SumWithSumOfSqrs
    },
    engine::{FlatAlgorithm, FlatWaveform, MetricProperty},
    event::ChannelData,
};
use serde::{Deserialize, Serialize};

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
    lifetime: MeanSD,
}

impl CompleteMetricResultClass for CompletedMuonLifetime {
    type Partial = MuonLifetime;

    fn aggregate(source: &Self::Partial) -> Self {
        Self {
            lifetime: source.lifetime.mean_and_stddev(source.num as f64),
        }
    }

    fn get_property(&self, property: &MetricProperty) -> Result<MetricOutput<f64>, String> {
        match property {
            MetricProperty::Mean => Ok(MetricOutput::Scalar(self.lifetime.mean)),
            MetricProperty::SD => Ok(MetricOutput::ScalarWithBand(
                self.lifetime.mean,
                self.lifetime.sd,
            )),
            _ => unreachable!(),
        }
    }
}
