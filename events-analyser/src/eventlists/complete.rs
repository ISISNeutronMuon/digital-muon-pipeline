//! Defines the struct for a frame which is awaiting data from digitiser messages.
//!
use crate::event::{ChannelData, EventData};
use digital_muon_common::{
    Channel, DigitizerId,
    spanned::{SpanOnce, Spanned},
};
use digital_muon_streaming_types::FrameMetadata;
use std::collections::HashMap;

pub(crate) type ChannelDataByTopic = Vec<ChannelData>;
pub(crate) type ChannelCollection = HashMap<Channel, ChannelDataByTopic>;

pub(crate) struct EventlistsCollection {
    /// Used by the implementation of [SpannedAggregator].
    ///
    /// [SpannedAggregator]: digital_muon_common::spanned::SpannedAggregator
    pub(crate) span: SpanOnce,
    /// The digitise id of the message.
    pub(crate) digitiser_id: DigitizerId,
    /// The identifying metadata of the message, common to all digitiser messages related to this frame (except possibly for [FrameMetadata::veto_flags]).
    pub(crate) metadata: FrameMetadata,
    /// The event data from each topic.
    pub(crate) eventlists: Vec<EventData>,
    /// Channels.
    pub(crate) channels: Vec<Channel>,
}

impl EventlistsCollection {
    pub(crate) fn new(
        span: SpanOnce,
        digitiser_id: DigitizerId,
        metadata: FrameMetadata,
        eventlists: Vec<EventData>,
    ) -> Self {
        let mut channels = eventlists
            .iter()
            .flat_map(|eventlist| eventlist.get_channels())
            .collect::<Vec<_>>();
        channels.sort();
        channels.dedup();
        Self {
            span,
            digitiser_id,
            metadata,
            eventlists,
            channels,
        }
    }

    pub(crate) fn into_channel_collection(self) -> ChannelCollection {
        let mut temp = ChannelCollection::new();
        let default = vec![Default::default(); self.eventlists.len()];
        for (topic_index, event_data) in self.eventlists.into_iter().enumerate() {
            for (channel, channel_data) in event_data.events.into_iter() {
                *temp
                    .entry(channel)
                    .or_insert_with(|| default.clone())
                    .get_mut(topic_index)
                    .expect("This should never fail.") = channel_data
            }
        }
        temp
    }
}

impl Spanned for EventlistsCollection {
    fn span(&self) -> &SpanOnce {
        &self.span
    }
}
