//! Defines and implements the objects represent frames, as well as the cache to store them.
//!
//! The data stored in each frame is abstracted as a generic type and
//! defined in the [crate::data] module.
mod cache;
mod partial;

pub(crate) use cache::MessageCache;
pub(crate) use partial::{EventlistsCollection, PartialEventslistsCollection};

/// Represents the reason why a digitiser event list message is rejected
pub(crate) enum RejectMessageError {
    /// The frame has already encountered an event list from this digitiser.
    IdAlreadyPresent,
    /// The event list's timestamp occurs before [FrameCache::latest_timestamp_dispatched].
    TimestampTooEarly,
}

impl From<RejectMessageError> for &'static str {
    fn from(value: RejectMessageError) -> Self {
        match value {
            RejectMessageError::IdAlreadyPresent => "id_already_present",
            RejectMessageError::TimestampTooEarly => "timestamp_too_early",
        }
    }
}
