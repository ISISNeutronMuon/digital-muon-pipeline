//! Provides iterators to convert raw trace data into events and pulses.
pub(crate) mod event;

use super::{Assembler, Detector, Pulse, TracePoint};
pub(crate) use event::{AssembleFilter, EventFilter};
