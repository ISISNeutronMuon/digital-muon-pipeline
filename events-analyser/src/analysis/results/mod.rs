mod complete;
mod partial;

use std::ops::Deref;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub(crate) use complete::CompletedMetricResult;
pub(crate) use partial::PartialMetricResult;

/// Encapulates an object implementing `MetricResultBucket`, as well as
/// the number of messages that have been pushed to it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub(crate) struct MetricResultBucketWrapper<T> {
    /// Number of messages stored in this bucket.
    pub(crate) num_messages: usize,
    /// Underlying results storage object.
    pub(crate) object: T,
}

impl<T> Deref for MetricResultBucketWrapper<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.object
    }
}

/// A generic type which stores results of a metric by bucket.
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "P: Serialize + DeserializeOwned")]
pub(crate) struct MetricResultByBucket<P> {
    /// Metric results are stored by bucket and bucket block, that is the
    /// inner and outer `Vec`` is bucket and bucket block respectively.
    by_bucket: Vec<Vec<MetricResultBucketWrapper<P>>>,
}
