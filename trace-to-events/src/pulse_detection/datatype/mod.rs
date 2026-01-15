//! Provides abstractions for data types used in both traces and eventlists.
use super::Real;
use std::fmt::{Debug, Display};

//pub(crate) mod eventdata;
mod event;
//pub(crate) mod tracepoint;
mod trace;

//pub(crate) use eventdata::EventData;
pub(crate) use event::{EventData, EventPoint};
//pub(crate) use tracepoint::;
pub(crate) use trace::{RealArray, Stats, TraceArray, TracePoint};

/// This trait abstracts any type used as a time variable.
pub(crate) trait Temporal: Default + Copy + Debug + Display + PartialEq {}

impl Temporal for Real {}
