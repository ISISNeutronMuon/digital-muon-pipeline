mod complete;
mod partial;

use crate::analysis::metrics::MetricResultClass;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub(crate) use complete::CompletedMetricResult;
pub(crate) use partial::PartialMetricResult;

/// Type which stores metric results by bucket within a block.
type BucketStore<C> = Vec<C>;

/// Type which stores metric results by bucket block.
type BucketBlockStore<C> = Vec<BucketStore<C>>;

/// A generic type which stores 
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "C: Serialize + DeserializeOwned")]
pub(crate) struct MetricResultStore<C>
where
    C: MetricResultClass,
{
    /// Metric results storage by bucket block and bucket.
    by_bucket: BucketBlockStore<C>,
}
