//! Defines the struct for a frame which is awaiting data from digitiser messages.
use digital_muon_common::{
    Channel, DigitizerId,
    spanned::{SpanOnce, SpanOnceError, Spanned, SpannedAggregator, SpannedMut},
};
use digital_muon_streaming_types::FrameMetadata;
use std::{collections::HashMap, time::Duration};
use tokio::time::Instant;
use tracing::{Span, debug, info_span};

use crate::event::{ChannelData, EventData};

#[derive(Debug)]
pub(crate) struct EventlistsCollection {
    pub(crate) digitiser_id: DigitizerId,
    /// The uniquely identifying metadata of the frame, common to all digitiser messages related to this frame (except possibly for [FrameMetadata::veto_flags]).
    pub(crate) metadata: FrameMetadata,
    /// The frame's event data.
    eventlists: Vec<EventData>,
    /// Channels.
    pub(crate) channels: Vec<Channel>,
}

impl EventlistsCollection {
    fn new(digitiser_id: DigitizerId, metadata: FrameMetadata, eventlists: Vec<EventData>) -> Self {
        let mut channels = eventlists
            .iter()
            .flat_map(|eventlist| eventlist.get_channels())
            .collect::<Vec<_>>();
        channels.sort();
        channels.dedup();
        Self {
            digitiser_id,
            metadata,
            eventlists,
            channels,
        }
    }

    pub(crate) fn into_channel_collection(self) -> HashMap<Channel, Vec<ChannelData>> {
        let mut temp = HashMap::<Channel, Vec<ChannelData>>::new();
        let default = vec![Default::default(); self.eventlists.len()];
        for (topic_index, event_data) in self.eventlists.into_iter().enumerate() {
            for (channel, channel_data) in event_data.events.into_iter() {
                *temp
                    .entry(channel)
                    .or_insert_with(|| default.clone())
                    .get_mut(topic_index)
                    .expect("") = channel_data
            }
        }
        temp
    }
}

/// Holds the data of a frame, whislt it is in cache being built from digitiser messages.
pub(crate) struct PartialEventslistsCollection {
    /// Used by the implementation of [SpannedAggregator].
    ///
    /// [SpannedAggregator]: digital_muon_common::spanned::SpannedAggregator
    span: SpanOnce,
    /// IS `true` if and only if all expected digitiser messages have been collected.
    complete: bool,
    /// Time at which the partial frame should be considered expired, and can be dispatched
    /// from the cache even if incomplete.
    expiry: Instant,
    pub(super) digitiser_id: DigitizerId,
    /// The uniquely identifying metadata of the frame, common to all digitiser messages related to this frame (except possibly for [FrameMetadata::veto_flags]).
    pub(super) metadata: FrameMetadata,
    /// The frame's event data.
    eventlists: Vec<Option<EventData>>,
}

impl PartialEventslistsCollection {
    pub(super) fn new(
        num_topics: usize,
        ttl: Duration,
        metadata: &FrameMetadata,
        digitiser_id: DigitizerId,
    ) -> Self {
        let expiry = Instant::now() + ttl;
        let mut eventlists = Vec::with_capacity(num_topics);
        eventlists.resize_with(num_topics, || None);
        Self {
            span: SpanOnce::default(),
            complete: false,
            expiry,
            digitiser_id,
            metadata: metadata.clone(),
            eventlists,
        }
    }

    /// Sets the [self.complete] flag to true only if [Self::digitiser_ids] returns
    /// a list equal to the given `expected_digitisers`.
    /// Note that `expected_digitisers` must be increasing and non-repeating, otherwise the
    /// [self.complete] flag is never set. This is not checked, and left to the user.
    ///
    /// [self.complete]: Self::complete
    pub(super) fn set_completion_status(&mut self) {
        if self.eventlists.iter().all(Option::is_some) {
            self.complete = true;
        }
    }

    /// Returns `true` if and only if this provided [DigitizerId] has been seen before.
    pub(super) fn has_topic(&self, topic_index: usize) -> bool {
        self.eventlists.get(topic_index).expect("").is_some()
    }

    /// Pushes the given data from a digitser to the frame.
    /// # Parameters
    /// - digitiser_id: the id of the digitiser sending the data.
    /// - data: the data in the message.
    pub(crate) fn push(&mut self, topic_index: usize, data: EventData) {
        *self
            .eventlists
            .get_mut(topic_index)
            .expect("topic_index should not be too large, this should never fail.") = Some(data);
        self.set_completion_status();
        debug!("Completed: {}", self.complete);
    }

    /// Ammends the metadata [veto_flags] field with `veto_flags` from a new digitiser message.
    /// This is necessary until it is determined whether [veto_flags] should be identical accross
    /// all digitisers during a frame.
    ///
    /// [veto_flags]: FrameMetadata::veto_flags
    pub(super) fn push_veto_flags(&mut self, veto_flags: u16) {
        self.metadata.veto_flags |= veto_flags;
    }

    /// Returns value of [self.complete].
    ///
    /// [self.complete]: Self::complete
    pub(super) fn is_complete(&self) -> bool {
        self.complete
    }

    /// Returns `true` if and only if the current time instant is greater than `self.expiry`
    pub(super) fn is_expired(&self) -> bool {
        Instant::now() > self.expiry
    }

    pub(crate) fn try_complete(self) -> Option<EventlistsCollection> {
        self.eventlists
            .into_iter()
            .collect::<Option<Vec<EventData>>>()
            .map(|eventlists| {
                EventlistsCollection::new(self.digitiser_id, self.metadata, eventlists)
            })
    }
}

impl Spanned for PartialEventslistsCollection {
    fn span(&self) -> &SpanOnce {
        &self.span
    }
}

impl SpannedMut for PartialEventslistsCollection {
    fn span_mut(&mut self) -> &mut SpanOnce {
        &mut self.span
    }
}

impl SpannedAggregator for PartialEventslistsCollection {
    fn span_init(&mut self) -> Result<(), SpanOnceError> {
        self.span.init(info_span!(parent: None, "Frame",
            "metadata_timestamp" = self.metadata.timestamp.to_rfc3339(),
            "metadata_frame_number" = self.metadata.frame_number,
            "metadata_period_number" = self.metadata.period_number,
            "metadata_veto_flags" = self.metadata.veto_flags,
            "metadata_protons_per_pulse" = self.metadata.protons_per_pulse,
            "metadata_running" = self.metadata.running,
            "frame_is_expired" = tracing::field::Empty,
        ))
    }

    fn link_current_span<F: Fn() -> Span>(
        &self,
        aggregated_span_fn: F,
    ) -> Result<(), SpanOnceError> {
        let span = self.span.get()?.in_scope(aggregated_span_fn);
        span.follows_from(tracing::Span::current());
        Ok(())
    }

    fn end_span(&self) -> Result<(), SpanOnceError> {
        #[cfg(not(test))] //   In test mode, the frame.span() are not initialised
        self.span()
            .get()?
            .record("frame_is_expired", self.is_expired() && !self.is_complete());
        Ok(())
    }
}
