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

pub(crate) trait TaskClass {}

pub(crate) struct SearchTask<'a, C: TaskClass> {
    consumer: &'a StreamConsumer,
    topics: &'a Topics,
    events_topic_index: usize,
    phantom: PhantomData<C>,
}

impl<'a, C: TaskClass> SearchTask<'a, C> {
    pub(crate) fn new(
        consumer: &'a StreamConsumer,
        topics: &'a Topics,
        events_topic_index: usize,
    ) -> Self {
        Self {
            consumer,
            topics,
            events_topic_index,
            phantom: PhantomData,
        }
    }

    /// Extracts a sorted, deduplicated vector of Digitiser Ids from a slice of trace messages.
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
