mod error;
mod metrics;
mod outputs;
mod utils;

use crate::{
    engine::{Flattenable, ValueError},
    eventlists::ChannelDataByTopic,
};
use digital_muon_common::Channel;
use digital_muon_streaming_types::FrameMetadata;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

pub(crate) use error::{FittingError, MetricResultError};
pub(crate) use metrics::{
    ListOfMetrics, MetricWrapper, MetricWrapperWithTag, complete, dependent, partial, properties,
    resolved,
};
pub(crate) use outputs::{CompleteSeriesOutput, HistogramWithBands, LabelledGroupData, Surface};

pub(crate) trait DependentMetric: Clone + Debug {
    type Resolved: ResolvedMetric<Dependent = Self>;
    type Independent: Clone + Debug + Serialize + DeserializeOwned;

    fn resolve_metric(&self, library: &[String]) -> Result<Self::Resolved, ValueError>;
}

/// Defines a type which describes a metric, and is created by deserialization from the JSON configuaration file.
/// If consists of two parts, the [Self::json] field, which requires `flattening`, and the [Self::settings] field,
/// which does not.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub(crate) struct DependentMetricShell<T: DependentMetric> {
    /// Represents fields which are dependent on other parts of the configuartion file.
    #[serde(flatten)]
    dependent: T,
    /// Represents fields which are not dependent on other parts of the configuartion file.
    #[serde(flatten)]
    independent: T::Independent,
}

pub(crate) trait ResolvedMetric: Clone + Debug + Serialize + DeserializeOwned {
    type Dependent: DependentMetric<Resolved = Self>;

    #[cfg(test)]
    /// Used for testing.
    fn to_shell(
        self,
        independent: <Self::Dependent as DependentMetric>::Independent,
    ) -> ResolvedMetricShell<Self> {
        ResolvedMetricShell {
            resolved: self,
            independent,
        }
    }
}

/// Defines a type, derived from an instance of [ResolvedMetricShell] which has had its dependencies resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub(crate) struct ResolvedMetricShell<T: ResolvedMetric> {
    /// Represents fields which have been flattened to resolve their dependencies on other parts of the configuartion file.
    resolved: T,
    /// Represents fields which are not dependent on other parts of the configuartion file, and have been copied straight from [DependentMetricShell].
    #[serde(flatten)]
    independent: <T::Dependent as DependentMetric>::Independent,
}

impl<T: ResolvedMetric> Deref for ResolvedMetricShell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.resolved
    }
}

impl<T> Flattenable<&[String]> for DependentMetricShell<T>
where
    T: DependentMetric,
{
    type Flat = ResolvedMetricShell<T::Resolved>;
    type Error = ValueError;

    fn flatten(&self, library: &[String]) -> Result<Self::Flat, Self::Error> {
        Ok(ResolvedMetricShell {
            resolved: self.dependent.resolve_metric(library)?,
            independent: self.independent.clone(),
        })
    }
}

pub(crate) trait PartialMetric: Default + Clone {
    type Source: ResolvedMetric;

    fn push(
        &mut self,
        source: &ResolvedMetricShell<Self::Source>,
        channel: Channel,
        metadata: &FrameMetadata,
        collection_by_topic: &ChannelDataByTopic,
    );
    fn load_data(&mut self, data_source: &Self) {
        *self = data_source.clone();
    }

    #[cfg(test)]
    fn to_shell(
        self,
        source: ResolvedMetricShell<Self::Source>,
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
    source: ResolvedMetricShell<P::Source>,
    notifications: Vec<String>,
    inner: P,
}

impl<P: PartialMetric> PartialMetricShell<P> {
    pub(crate) fn new(source: &ResolvedMetricShell<P::Source>) -> Self {
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

    pub(crate) fn push(
        &mut self,
        channel: Channel,
        metadata: &FrameMetadata,
        collection_by_topic: &ChannelDataByTopic,
    ) {
        self.inner
            .push(&self.source, channel, metadata, collection_by_topic)
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
