//! Defines the struct for a frame which is awaiting data from digitiser messages.
use crate::data::DigitiserData;
use digital_muon_common::{
    DigitizerId, FrameNumber, spanned::{SpanOnce, SpanOnceError, Spanned, SpannedAggregator, SpannedMut}
};
use digital_muon_streaming_types::FrameMetadata;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{Span, info_span};

/// Holds the data of a frame, whislt it is in cache being built from digitiser messages.
pub(crate) struct FrameDigitiserEventsLists<D> {
    /// Used by the implementation of [SpannedAggregator].
    ///
    /// [SpannedAggregator]: digital_muon_common::spanned::SpannedAggregator
    span: SpanOnce,
    /// IS `true` if and only if all expected digitiser messages have been collected.
    complete: bool,
    /// Time at which the partial frame should be considered expired, and can be dispatched
    /// from the cache even if incomplete.
    expiry: Instant,
    frame_number: FrameNumber,
    pub(super) digitiser_id: DigitizerId,
    /// The uniquely identifying metadata of the frame, common to all digitiser messages related to this frame (except possibly for [FrameMetadata::veto_flags]).
    pub(super) metadata: FrameMetadata,
    /// The frame's event data.
    eventlists: Vec<Option<D>>
}

impl<D> FrameDigitiserEventsLists<D> {
    pub(super) fn new(num_topics: usize, ttl: Duration, metadata: FrameMetadata, frame_number: FrameNumber, digitiser_id: DigitizerId,) -> Self {
        let expiry = Instant::now() + ttl;
        let mut eventlists = Vec::with_capacity(num_topics);
        eventlists.resize_with(num_topics, ||None);
        Self {
            span: SpanOnce::default(),
            complete: false,
            expiry,
            frame_number,
            digitiser_id,
            metadata,
            eventlists
        }
    }

    /// Sets the [self.complete] flag to true only if [Self::digitiser_ids] returns
    /// a list equal to the given `expected_digitisers`.
    /// Note that `expected_digitisers` must be increasing and non-repeating, otherwise the
    /// [self.complete] flag is never set. This is not checked, and left to the user.
    ///
    /// [self.complete]: Self::complete
    pub(super) fn set_completion_status(&mut self) {
        if self.eventlists.iter().all(|eventlist|eventlist.is_some()) {
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
    pub(crate) fn push(&mut self, topic_index: usize, data: D) {
        self.eventlists.insert(topic_index, Some(data));
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
}

impl<D> Spanned for FrameDigitiserEventsLists<D> {
    fn span(&self) -> &SpanOnce {
        &self.span
    }
}

impl<D> SpannedMut for FrameDigitiserEventsLists<D> {
    fn span_mut(&mut self) -> &mut SpanOnce {
        &mut self.span
    }
}

impl<D> SpannedAggregator for FrameDigitiserEventsLists<D> {
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
