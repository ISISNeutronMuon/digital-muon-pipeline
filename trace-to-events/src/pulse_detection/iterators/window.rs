use crate::pulse_detection::{TracePoint, window::Window};


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
pub(crate) trait WindowIterable<I, W>
where
    I: Iterator,
    I::Item: TracePoint,
    W: Window,
{
    /// Creates an iterator which applies a window to the iterator.
    fn window(self, window: W) -> WindowIter<I, W>;
}

impl<I, W> WindowIterable<I, W> for I
where
    I: Iterator,
    I::Item: TracePoint,
    W: Window,
{
    fn window(self, window: W) -> WindowIter<I, W> {
        WindowIter::<I, W>::new(self, window)
    }
}
