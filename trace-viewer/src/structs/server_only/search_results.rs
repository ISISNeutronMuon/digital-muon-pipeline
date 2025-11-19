use crate::{
    app::SessionError,
    structs::digitiser_messages::{
        DigitiserEventList, DigitiserMetadata, DigitiserTrace, FromMessage,
    },
};
use digital_muon_streaming_types::{
    dat2_digitizer_analog_trace_v2_generated::DigitizerAnalogTraceMessage,
    dev2_digitizer_event_v2_generated::DigitizerEventListMessage,
    time_conversions::GpsTimeConversionError,
};
use std::collections::{
    BTreeMap,
    btree_map::{self, Entry},
};
use tracing::{error, info};

/// Encapsulates the result of a completed search.
#[derive(Debug, Clone)]
pub(crate) enum SearchResults {
    /// The search was cancelled by the user.
    Cancelled,
    /// The search successfully returned results.
    Successful { cache: Cache },
}

impl SearchResults {
    /// Returns the underlying cache of the search results, if they exist, returns [SessionError::SearchCancelled] otherwise.
    pub fn cache(&self) -> Result<&Cache, SessionError> {
        match self {
            SearchResults::Cancelled => Err(SessionError::SearchCancelled),
            SearchResults::Successful { cache } => Ok(cache),
        }
    }
}

/// Stores trace and event list messages found by the searcher.
#[derive(Debug, Clone)]
pub struct Cache {
    /// Associative array which keys each found trace message by its metadata.
    traces: BTreeMap<DigitiserMetadata, DigitiserTrace>,
    /// Associative array of associative arrays. The top level key is the index of the eventlist topic,
    /// to which the found eventlists belong. The lower level key is the metadata of the found eventlist.
    events: BTreeMap<usize, BTreeMap<DigitiserMetadata, DigitiserEventList>>,
}

impl Cache {
    pub(crate) fn new() -> Self {
        Self {
            traces: Default::default(),
            events: Default::default(),
        }
    }

    /// Push a trace message to the cache.
    #[tracing::instrument(skip_all)]
    pub(crate) fn push_trace(
        &mut self,
        msg: &DigitizerAnalogTraceMessage<'_>,
    ) -> Result<(), GpsTimeConversionError> {
        let metadata = DigitiserMetadata {
            id: msg.digitizer_id(),
            timestamp: msg
                .metadata()
                .timestamp()
                .copied()
                .expect("Timestamp should exist.")
                .try_into()?,
            frame_number: msg.metadata().frame_number(),
            period_number: msg.metadata().period_number(),
            protons_per_pulse: msg.metadata().protons_per_pulse(),
            running: msg.metadata().running(),
            veto_flags: msg.metadata().veto_flags(),
        };

        match self.traces.entry(metadata) {
            Entry::Occupied(occupied_entry) => {
                error!("Trace already found: {0:?}", occupied_entry.key());
            }
            Entry::Vacant(vacant_entry) => {
                info!("Trace Entered: {:?}", vacant_entry.key());
                vacant_entry.insert(DigitiserTrace::from_message(msg));
            }
        }
        Ok(())
    }

    pub(crate) fn iter(&self) -> btree_map::Iter<'_, DigitiserMetadata, DigitiserTrace> {
        self.traces.iter()
    }

    /// Push an event list to the cache, along with the eventlist topic index it belongs to.
    #[tracing::instrument(skip_all)]
    pub(crate) fn push_events(
        &mut self,
        topic_index: usize,
        msg: &DigitizerEventListMessage<'_>,
    ) -> Result<(), GpsTimeConversionError> {
        let metadata = DigitiserMetadata {
            id: msg.digitizer_id(),
            timestamp: msg
                .metadata()
                .timestamp()
                .copied()
                .expect("Timestamp should exist.")
                .try_into()?,
            frame_number: msg.metadata().frame_number(),
            period_number: msg.metadata().period_number(),
            protons_per_pulse: msg.metadata().protons_per_pulse(),
            running: msg.metadata().running(),
            veto_flags: msg.metadata().veto_flags(),
        };
        let events = self.events.entry(topic_index).or_default();
        match events.entry(metadata) {
            Entry::Occupied(occupied_entry) => {
                error!("Event list already found: {0:?}", occupied_entry.key());
            }
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(DigitiserEventList::from_message(msg));
            }
        }
        Ok(())
    }

    /// Traverse the eventlists in the cache and assign associate them
    /// to the trace messages with matching metadata, if one exists.
    pub(crate) fn attach_event_lists_to_trace(&mut self) {
        for (&topic, events) in &self.events {
            for (metadata, events) in events {
                match self.traces.entry(metadata.clone()) {
                    Entry::Occupied(mut occupied_entry) => {
                        info!("Found Trace for Events");
                        occupied_entry
                            .get_mut()
                            .events
                            .insert(topic, events.clone());
                    }
                    Entry::Vacant(vacant_entry) => {
                        error!("Trace not found: {0:?}", vacant_entry.key());
                    }
                }
            }
        }
    }

    /// Returns iterator to the eventlist topic indices which exist in the cache.
    pub(crate) fn get_eventlist_topic_indices(&self) -> impl Iterator<Item = &usize> {
        self.events.keys()
    }
}
