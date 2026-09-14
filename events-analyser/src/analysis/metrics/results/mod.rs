mod complete;
mod partial;

use std::ops::Deref;

use crate::analysis::metrics::FittingError;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

pub(crate) use complete::{CompleteMetricResultClass, CompletedMetricResult};
pub(crate) use partial::{PartialMetricResult, PartialMetricResultClass};

/// Type which stores metric results by bucket within a block.
type BucketStore<C> = Vec<C>;

/// Type which stores metric results by bucket block.
type BucketBlockStore<C> = Vec<BucketStore<MetricObject<C>>>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MetricObject<C> {
    pub(crate) num_messages: usize,
    pub(crate) object: C,
}

impl<C> Deref for MetricObject<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.object
    }
}

/// A generic type which stores
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "C: Serialize + DeserializeOwned")]
pub(crate) struct MetricResultByBucket<C>
where
    C: Clone + Serialize + DeserializeOwned,
{
    /// Metric results storage by bucket block and bucket.
    by_bucket: BucketBlockStore<C>,
}

impl<C> MetricResultByBucket<C>
where
    C: PartialMetricResultClass,
{
    pub(crate) fn load_data(&mut self, source: &Self) {
        for (bucket, source_bucket) in Iterator::zip(
            self.by_bucket.iter_mut().flatten(),
            source.by_bucket.iter().flatten(),
        ) {
            bucket.num_messages = source_bucket.num_messages;
            bucket.object.load_data(&source_bucket.object);
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum MetricResultError {
    #[error("{0}")]
    Fitting(#[from] FittingError),
    #[error("Unable to load data from saved metrics file.")]
    LoadingDataWrongMetrics,
}
