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

use super::{Real, RealArray, Stats, Temporal, TracePoint};
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

/// Iterator which applies a window to another iterator.
#[derive(Clone)]
pub(crate) struct WindowIter<I, W>
where
    I: Iterator,
    I::Item: TracePoint,
    W: Window,
{
    /// Window function to apply.
    window_function: W,
    /// Source.
    source: I,
}

impl<I, W> WindowIter<I, W>
where
    I: Iterator,
    I::Item: TracePoint,
    W: Window,
{
    /// Creates a new iterator which applies the given window.
    ///
    /// # Parameters
    /// - source: base iterator which is consumed.
    /// - window_function: window to apply to the base iterator.
    pub fn new(source: I, window_function: W) -> Self {
        WindowIter {
            source,
            window_function,
        }
    }

    #[cfg(test)]
    pub fn get_window(&self) -> &W {
        &self.window_function
    }
}

impl<I, W> Iterator for WindowIter<I, W>
where
    I: Iterator,
    I::Item: TracePoint,
    W: Window<TimeType = <I::Item as TracePoint>::Time, InputType = <I::Item as TracePoint>::Value>,
{
    type Item = (W::TimeType, W::OutputType);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let val = self.source.next()?;
            if self.window_function.push(val.get_value().clone()) {
                return Some((
                    self.window_function.apply_time_shift(val.get_time()),
                    self.window_function.output()?,
                ));
            }
        }
    }
}

/// Provides method for creating a window iterator from another iterator.
pub(crate) trait WindowFilter<I, W>
where
    I: Iterator,
    I::Item: TracePoint,
    W: Window,
{
    /// Creates an iterator which applies a window to the iterator.
    fn window(self, window: W) -> WindowIter<I, W>;
}

impl<I, W> WindowFilter<I, W> for I
where
    I: Iterator,
    I::Item: TracePoint,
    W: Window,
{
    fn window(self, window: W) -> WindowIter<I, W> {
        WindowIter::<I, W>::new(self, window)
    }
}
