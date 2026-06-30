mod event_counts;
mod false_counts;
mod group_by;
mod muon_lifetime;
mod output;
mod results;

use crate::{
    engine::{FlatAlgorithm, FlatWaveform, MetricProperty},
    event::ChannelData,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub(crate) use output::MetricOutput;
pub(crate) use results::{CompletedMetricResult, PartialMetricResult};

/// Holds the running sum of a sequence, as well as the sum of squares.
/// These are used to compute mean and standard deviations once the sums are complete.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
struct SumWithSumOfSqrs {
    /// The number of values added into the sums.
    num: f64,
    /// The sum of the sequence.
    sum: f64,
    /// The sum of the squares of the sequence.
    sqr_sum: f64,
}

impl SumWithSumOfSqrs {
    /// Adds a sequence value to the 
    fn add_to(&mut self, value: f64) {
        self.num += 1.0;
        self.sum += value;
        self.sqr_sum += value * value;
    }

    pub(crate) fn mean_and_stddev(&self) -> MeanSD {
        MeanSD {
            mean: self.sum / self.num,
            sd: f64::sqrt((self.num * self.sqr_sum - self.sum * self.sum) / (self.num * (self.num - 1.0))),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MeanSD {
    pub(crate) mean: f64,
    pub(crate) sd: f64,
}

pub(crate) trait MetricResultClass: Clone + Serialize + DeserializeOwned {}

impl<T> MetricResultClass for T where T: Clone + Serialize + DeserializeOwned {}

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
