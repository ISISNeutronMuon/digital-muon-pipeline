mod group_data;
mod histogram;

use serde::{Deserialize, Serialize};

pub(crate) use group_data::GroupDataBy;
pub(crate) use histogram::Histogram;

/// Holds the running sum of a sequence, as well as the sum of squares.
/// These are used to compute mean and standard deviations once the sums are complete.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SumWithSumOfSqrs {
    /// The number of values added into the sums.
    num: f64,
    /// The sum of the sequence.
    sum: f64,
    /// The sum of the squares of the sequence.
    sqr_sum: f64,
}

impl SumWithSumOfSqrs {
    /// Adds a sequence value to the
    pub(crate) fn add_to(&mut self, value: f64) {
        self.num += 1.0;
        self.sum += value;
        self.sqr_sum += value * value;
    }

    pub(crate) fn compose_with(mut self, value: &SumWithSumOfSqrs) -> Self {
        self.num += value.num;
        self.sum += value.sum;
        self.sqr_sum += value.sqr_sum;
        self
    }

    pub(crate) fn mean_and_stddev(&self) -> MeanSD {
        MeanSD {
            mean: self.sum / self.num,
            sd: f64::sqrt(
                (self.num * self.sqr_sum - self.sum * self.sum) / (self.num * (self.num - 1.0)),
            ),
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MeanSD {
    pub(crate) mean: f64,
    pub(crate) sd: f64,
}

impl<'a> FromIterator<&'a MeanSD> for Option<MeanSD> {
    fn from_iter<T: IntoIterator<Item = &'a MeanSD>>(iter: T) -> Self {
        let (sum, sd, len) = iter.into_iter().fold((0.0, 0.0, 0), |sum, next| {
            (sum.0 + next.mean, sum.1 + next.sd, sum.2 + 1)
        });
        if len == 0 {
            None
        } else {
            Some(MeanSD {
                mean: sum / len as f64,
                sd,
            })
        }
    }
}
