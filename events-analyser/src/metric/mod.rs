mod error;
mod event_count;
mod muon_lifetime;
mod outputs;
mod utils;

use digital_muon_common::Channel;
use utils::Histogram;
use crate::{engine::Flattenable, eventlists::ChannelDataByTopic, metric::error::MetricResultError};

pub(crate) mod json {
    pub(crate) use super::event_count::JsonEventCount as EventCount;
    pub(crate) use super::muon_lifetime::JsonMuonLifetime as MuonLifetime;
}

pub(crate) mod flat {
    pub(crate) use super::event_count::FlatEventCount as EventCount;
    pub(crate) use super::muon_lifetime::FlatMuonLifetime as MuonLifetime;
}

pub(crate) mod partial {
    pub(crate) use super::event_count::PartialEventCount as EventCount;
    pub(crate) use super::muon_lifetime::PartialMuonLifetime as MuonLifetime;
}

pub(crate) mod complete {
    pub(crate) use super::event_count::CompletedEventCount as EventCount;
    pub(crate) use super::muon_lifetime::CompletedMuonLifetime as MuonLifetime;
}

pub(crate) trait JsonMetric {
    type Flattened: FlattenedMetric<Json = Self>;
    type Settings : Clone;

    fn flatten_metric(&self, library: &[String]) -> Result<Self::Flattened, ()>;
}

pub(crate) trait FlattenedMetric : Clone {
    type Json: JsonMetric<Flattened = Self>;
}

impl<T> Flattenable<&[String]> for T where T: JsonMetric {
    type Flat = T::Flattened;
    type Error = ();

    fn flatten(&self, library: &[String]) -> Result<Self::Flat, Self::Error> {
        self.flatten_metric(library)
    }
}

pub(crate) trait PartialMetric: Default {
    type Source: FlattenedMetric;

    fn push(&mut self, source: &Self::Source, channel: Channel, collection_by_topic: &ChannelDataByTopic);
}

pub(crate) struct PartialMetricShell<P : PartialMetric> {
    source: P::Source,
    inner: P,
}

impl<P : PartialMetric> PartialMetricShell<P> {
    pub(crate) fn make_default(source: &P::Source) -> Self {
        Self {
            source: source.clone(),
            inner: Default::default(),
        }
    }

    pub(crate) fn push(&mut self, channel: Channel, collection_by_topic: &ChannelDataByTopic) {
        self.inner.push(&self.source, channel, collection_by_topic)
    }
}

pub(crate) trait CompleteMetric : Sized {
    /// The corresponding partial results type.
    type Partial: PartialMetric;
    /// Error type.
    type Error: Into<MetricResultError>;
    /// Type which specifies a particular property of the metric.
    type Property: Clone;
    type Output;

    /// Creates an instance of this object by aggregating the results from a partial results object.
    fn aggregate(source: &Self::Partial) -> Result<Self, Self::Error>;
    /// Extract a particular property of the results.
    fn get_property(&self, property: Self::Property) -> Result<Self::Output, Self::Error>;
}

pub(crate) trait Metric {
    type Json: JsonMetric<Flattened = Self::FlattenedMetric>;
    type FlattenedMetric : FlattenedMetric;
    type Partial : PartialMetric;
    type Complete : CompleteMetric;
}