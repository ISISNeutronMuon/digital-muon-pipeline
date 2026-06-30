use std::ops::AddAssign;

use crate::{
    analysis::metrics::{
        CompleteMetricResultClass, MeanSD, MetricOutput, PartialMetricResultClass, SumWithSumOfSqrs,
    },
    engine::{FlatAlgorithm, FlatMetricMuonLifetime, FlatWaveform, MetricProperty},
    event::ChannelData,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Histogram {
    bins: Vec<usize>,
    max_value: f64,
}

impl Histogram {
    pub(crate) fn new(num: usize, max_value: f64) -> Self {
        Self {
            bins: vec![Default::default(), num],
            max_value,
        }
    }

    pub(crate) fn push(&mut self, value: f64) {
        let index = ((value as f64/self.max_value)*self.bins.len() as f64) as usize;
        self.bins
            .get_mut(index)
            .expect("This should never fail")
            .add_assign(1);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MuonLifetime {
    num: usize,
    lifetime: SumWithSumOfSqrs,
    topic: usize,
    histogram: Histogram,
}

impl PartialMetricResultClass for MuonLifetime {
    type Source = FlatMetricMuonLifetime;
    type Complete = CompletedMuonLifetime;

    fn make_default(source: &FlatMetricMuonLifetime) -> Self {
        Self {
            num: Default::default(),
            lifetime: Default::default(),
            topic: source.topic,
            histogram: Histogram::new(source.num_bins, source.max_lifetime)
        }
    }

    fn push(
        &mut self,
        _waveform: &FlatWaveform,
        _algorithm: &FlatAlgorithm,
        by_topic: &[ChannelData],
    ) {
        self.num += 1;
        for (time, _) in by_topic.get(self.topic).expect("This should never fail.").get_time_intensity() {
            self.histogram.push(*time as f64);
        }
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
            lifetime: source.lifetime.mean_and_stddev(),
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
