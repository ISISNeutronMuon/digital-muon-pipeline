mod error;
mod metrics;
mod outputs;
mod utils;

use crate::{
    engine::{Flattenable, ValueError},
    eventlists::ChannelDataByTopic,
};
use digital_muon_common::Channel;
pub(crate) use error::{FittingError, MetricResultError};
pub(crate) use metrics::{ListOfMetrics, MetricWrapper, MetricWrapperWithTag, complete, flat, json, partial, properties};
pub(crate) use outputs::{CompleteSeriesOutput, HistogramWithBands, Surface};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

pub(crate) trait JsonMetric: Clone + Debug {
    type Flattened: FlattenedMetric<Json = Self>;
    type Settings: Clone + Debug + Serialize + DeserializeOwned;

    fn flatten_metric(&self, library: &[String]) -> Result<Self::Flattened, ValueError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub(crate) struct JsonMetricShell<T: JsonMetric> {
    #[serde(flatten)]
    json: T,
    #[serde(flatten)]
    settings: T::Settings,
}

pub(crate) trait FlattenedMetric: Clone + Debug + Serialize + DeserializeOwned {
    type Json: JsonMetric<Flattened = Self>;

    #[cfg(test)]
    fn to_shell(self, settings: <Self::Json as JsonMetric>::Settings) -> FlatMetricShell<Self> {
        FlatMetricShell {
            flat: self,
            settings,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub(crate) struct FlatMetricShell<T: FlattenedMetric> {
    flat: T,
    #[serde(flatten)]
    settings: <T::Json as JsonMetric>::Settings,
}

impl<T: FlattenedMetric> Deref for FlatMetricShell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.flat
    }
}

impl<T> Flattenable<&[String]> for JsonMetricShell<T>
where
    T: JsonMetric,
{
    type Flat = FlatMetricShell<T::Flattened>;
    type Error = ValueError;

    fn flatten(&self, library: &[String]) -> Result<Self::Flat, Self::Error> {
        Ok(FlatMetricShell {
            flat: self.json.flatten_metric(library)?,
            settings: self.settings.clone(),
        })
    }
}

pub(crate) trait PartialMetric: Default + Clone {
    type Source: FlattenedMetric;

    fn push(
        &mut self,
        source: &FlatMetricShell<Self::Source>,
        channel: Channel,
        collection_by_topic: &ChannelDataByTopic,
    );
    fn load_data(&mut self, data_source: &Self) {
        *self = data_source.clone();
    }

    #[cfg(test)]
    fn to_shell(
        self,
        source: FlatMetricShell<Self::Source>,
        notifications: Vec<String>,
    ) -> PartialMetricShell<Self> {
        PartialMetricShell {
            source,
            notifications,
            inner: self,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "P: Serialize + DeserializeOwned")]
pub(crate) struct PartialMetricShell<P: PartialMetric> {
    source: FlatMetricShell<P::Source>,
    notifications: Vec<String>,
    inner: P,
}

impl<P: PartialMetric> PartialMetricShell<P> {
    pub(crate) fn make_default(source: &FlatMetricShell<P::Source>) -> Self {
        Self {
            source: source.clone(),
            notifications: Default::default(),
            inner: Default::default(),
        }
    }

    pub(crate) fn load_data(&mut self, source: &Self) {
        self.source = source.source.clone();
        self.notifications = source.notifications.clone();
        self.inner.load_data(&source.inner);
    }

    pub(crate) fn push(&mut self, channel: Channel, collection_by_topic: &ChannelDataByTopic) {
        self.inner.push(&self.source, channel, collection_by_topic)
    }
}

impl<P: PartialMetric> Deref for PartialMetricShell<P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<P: PartialMetric> DerefMut for PartialMetricShell<P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub(crate) trait CompleteMetric: Sized {
    /// The corresponding partial results type.
    type Partial: PartialMetric;
    /// Error type.
    type Error: Into<MetricResultError>;
    /// Type which specifies a particular property of the metric.
    type Property: Clone;
    type Output;

    /// Creates an instance of this object by aggregating the results from a partial results object.
    fn aggregate(source: &PartialMetricShell<Self::Partial>) -> Result<Self, Self::Error>;
    /// Extract a particular property of the results.
    fn get_property(&self, property: Self::Property) -> Result<Self::Output, Self::Error>;
}
