mod complete;
mod partial;

use crate::analysis::metrics::MetricResultClass;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub(crate) use complete::CompletedMetricResult;
pub(crate) use partial::PartialMetricResult;

#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "C: Serialize + DeserializeOwned")]
pub(crate) struct MetricResultStore<C>
where
    C: MetricResultClass,
{
    by_bucket: Vec<Vec<C>>,
}
