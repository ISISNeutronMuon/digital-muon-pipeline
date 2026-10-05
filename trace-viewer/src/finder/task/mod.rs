//! Contains structs responsible for executing a particular search method.
mod binary_by_timestamp;
mod dragnet;

use crate::{
    DigitizerId,
    structs::{FBMessage, Topics, TraceMessage},
};
use rdkafka::consumer::StreamConsumer;
use std::marker::PhantomData;
use tracing::info;

pub(crate) use binary_by_timestamp::BinarySearchByTimestamp;
pub(crate) use dragnet::Dragnet;

/// Types implementing this serve to create subtypes of `SearchTask`,
/// that is `SearchTask<S>` where `S : TaskClass` is a subtype of
/// `SearchTask<T>` for `T` unbounded.
pub(crate) trait TaskClass {}

/// Performs a search of the broker on the trace topic, aswell as the specified eventlist topics.
/// The particular method of search is determined by the type `C`. Different such types have different
/// `impl` blocks providing different methods.
pub(crate) struct SearchTask<'a, C: TaskClass> {
    /// Kafka consumer object the engine uses to consume messages.
    consumer: &'a StreamConsumer,
    /// Lists of all known topics.
    topics: &'a Topics,
    /// Indices of the eventlist topics this task should search.
    events_topic_indices: Vec<usize>,
    phantom: PhantomData<C>,
}

impl<'a, C: TaskClass> SearchTask<'a, C> {
    /// Create a new task.
    ///
    /// # Parameters
    /// - consumer: the Kafka consumer object the engine uses to consume messages.
    /// - topics: lists of all known topics.
    /// - events_topic_indices: list of the specific eventlist topics this task should search.
    pub(crate) fn new(
        consumer: &'a StreamConsumer,
        topics: &'a Topics,
        events_topic_indices: Vec<usize>,
    ) -> Self {
        Self {
            consumer,
            topics,
            events_topic_indices,
            phantom: PhantomData,
        }
    }

    /// Extracts a sorted, deduplicated vector of Digitiser Ids from a slice of trace messages.
    ///
    /// # Parameters
    /// - traces: slice of the messages to extract the digitiser ids from.
    fn get_digitiser_ids_from_traces(traces: &[TraceMessage]) -> Vec<DigitizerId> {
        let mut digitiser_ids = traces
            .iter()
            .map(TraceMessage::digitiser_id)
            .collect::<Vec<_>>();
        digitiser_ids.sort();
        digitiser_ids.dedup();
        info!("Digitiser Id(s) derived: {digitiser_ids:?}");
        digitiser_ids
    }
}
