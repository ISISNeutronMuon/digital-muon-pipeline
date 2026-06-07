mod event_counts;
mod false_counts;
mod muon_lifetime;
mod output;
mod group_by;
mod result;

use crate::{
    engine::{FlatAlgorithm, FlatWaveform, MetricProperty},
    event::ChannelData,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub(crate) use output::MetricOutput;
pub(crate) use result::{PartialMetricResult, CompletedMetricResult};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
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

pub(crate) trait MetricResultClass: Clone + Serialize + DeserializeOwned {}

impl<T> MetricResultClass for T where T : Clone + Serialize + DeserializeOwned {}

pub(crate) trait PartialMetricResultClass: MetricResultClass {
    type Source;
    type Complete: CompleteMetricResultClass<Partial = Self>;

    fn make_default(source: &Self::Source) -> Self;
    fn push(
        &mut self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        by_topic: &[ChannelData],
    );
    fn len(&self) -> usize;
}

pub(crate) trait CompleteMetricResultClass: MetricResultClass {
    type Partial: PartialMetricResultClass<Complete = Self>;

    fn aggregate(source: &Self::Partial) -> Self;
    fn get_property(&self, property: &MetricProperty) -> Result<MetricOutput<f64>, String>;
}
