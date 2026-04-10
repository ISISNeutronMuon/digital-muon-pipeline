//! Provides abstractions for data types used in both traces and eventlists.
use super::Real;
use std::fmt::Debug;

mod event;
mod trace;

use digital_muon_common::Time;
pub(crate) use event::{EventData, EventPoint};
pub(crate) use trace::{RealArray, Stats, TraceArray, TracePoint};

/// This trait abstracts any type used as a time variable.
pub(crate) trait Temporal: Default + Copy + Debug + PartialEq {}

impl Temporal for usize {}
impl Temporal for Time {}
impl Temporal for Real {}
