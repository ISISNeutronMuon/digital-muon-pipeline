mod complete;
mod partial;

use std::ops::Deref;

use crate::analysis::metrics::FittingError;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

pub(crate) use complete::{CompleteMetricResultBucket, CompletedMetricResult};
pub(crate) use partial::{PartialMetricResult, PartialMetricResultBucket};

/// Encapulates an object implementing `MetricResultBucket`, as well as
/// the number of messages that have been pushed to it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MetricResultBucketWrapper<C> {
    /// Number of messages stored in this bucket.
    pub(crate) num_messages: usize,
    /// Underlying results storage object.
    pub(crate) object: C,
}

impl<C> Deref for MetricResultBucketWrapper<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.object
    }
}

/// A generic type which stores results of a metric by bucket.
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "C: Serialize + DeserializeOwned")]
pub(crate) struct MetricResultByBucket<C>
where
    C: Clone + Serialize + DeserializeOwned,
{
    /// Metric results are stored by bucket and bucket block, that is the
    /// inner and outer `Vec`` is bucket and bucket block respectively.
    by_bucket: Vec<Vec<MetricResultBucketWrapper<C>>>,
}

#[derive(Debug, Error)]
pub(crate) enum MetricResultError {
    #[error("{0}")]
    Fitting(#[from] FittingError),
    #[error("Unable to load data from saved metrics file.")]
    LoadingDataWrongMetrics,
}
