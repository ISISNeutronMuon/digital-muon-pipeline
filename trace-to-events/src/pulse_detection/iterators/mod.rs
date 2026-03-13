//! Provides iterators to convert raw trace data into events and pulses.
pub(crate) mod event;
pub(crate) mod padding;
pub(crate) mod window;

use super::{Detector, TracePoint};
pub(crate) use event::EventsIterable;
pub(crate) use padding::PaddingIterable;
pub(crate) use window::WindowIterable;
