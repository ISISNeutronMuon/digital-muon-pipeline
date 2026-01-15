//! Provides iterators to convert raw trace data into events and pulses.
pub(crate) mod event;
pub(crate) mod assembler;

use super::{Assembler, Detector, Pulse, TracePoint};
pub(crate) use event::EventFilter;
pub(crate) use assembler::AssembleFilter;
