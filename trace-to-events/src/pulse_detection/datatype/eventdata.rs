//! An abstraction of the time-independent types that are outputted by the various filters.
//!
//! [Todo] This modules can be combined with others for brevity
use std::fmt::{Debug, Display, Formatter, Result};

/// Abstracts of the types that represent values processed by the various filters
///
/// This differs from the EventPoint type in that EventData must represent a time value,
/// whereas TraceValue is time-agnostic.
pub(crate) trait EventData: Default + Clone + Debug + Display {}

#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub(crate) struct Empty {}

impl Display for Empty {
    fn fmt(&self, _f: &mut Formatter<'_>) -> Result {
        Ok(())
    }
}

impl EventData for Empty {}
