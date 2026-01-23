//! Defines [Window]s which perform operations on subintervals of a waveform.
//!
//! # Example
//!
//! The following example applies a baseline window, a smoothing window of length five,
//! and then a finite difference window to a raw data stream.
//! ```rust
//!     let smoothed = raw
//!        .window(Baseline::new(4, 0.1))
//!        .window(SmoothingWindow::new(5))
//!        .map(|(i, stats)| (i, stats.mean))
//!        .window(FiniteDifferences::<2>::new())
//!        .map(|(i,fd)| (i, fd[1]));
//! ```

pub(crate) mod baseline;
pub(crate) mod finite_differences;
pub(crate) mod smoothing_window;

use super::{Real, RealArray, Stats, Temporal};
pub(crate) use baseline::Baseline;
pub(crate) use finite_differences::FiniteDifferences;
pub(crate) use smoothing_window::SmoothingWindow;

/// Consumes values from a waveform, and outputs a waveform after processing.
pub(crate) trait Window: Clone {
    type TimeType: Temporal;
    type InputType: Copy;
    type OutputType;

    /// Pushes a value into the window.
    fn push(&mut self, value: Self::InputType) -> bool;

    /// Extracts the window's current processed value.
    fn output(&self) -> Option<Self::OutputType>;

    /// Shifts the time value by half the window's size.
    fn apply_time_shift(&self, time: Self::TimeType) -> Self::TimeType;
}
