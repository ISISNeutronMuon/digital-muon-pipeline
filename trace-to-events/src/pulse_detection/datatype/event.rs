//! An abstraction of both time-independent and time-dependent types that are outputted by the various filters.
//!
//! [Todo] This modules can be combined with others for brevity
use super::Temporal;
use std::fmt::Debug;

/// Abstracts of the types that represent values outputted by the various filters.
///
/// This differs from the EventPoint type in that EventData must represent a time value,
/// whereas TraceValue is time-agnostic.
pub(crate) trait EventData: Default + Clone + Debug {}

/// Abstracts types that are outputted by the various filters.
///
/// To implement this a type must contain time and event data.
pub(crate) trait EventPoint: Debug + Clone {
    type TimeType: Temporal;
    type EventType: EventData;

    fn get_time(&self) -> Self::TimeType;
    fn get_data(&self) -> &Self::EventType;
}

impl<T, E> EventPoint for (T, E)
where
    T: Temporal,
    E: EventData,
{
    type TimeType = T;
    type EventType = E;

    fn get_time(&self) -> T {
        self.0
    }

    fn get_data(&self) -> &E {
        &self.1
    }
}
