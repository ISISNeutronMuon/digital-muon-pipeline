//! Provides iterators to convert raw trace data into events and pulses.
pub(crate) mod event;
pub(crate) mod assembler;
pub(crate) mod window;

use super::{Assembler, Detector, Pulse, TracePoint};
pub(crate) use event::EventsIterable;
pub(crate) use assembler::AssembleIterable;
pub(crate) use window::WindowIterable;
