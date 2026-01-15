//! Provides event iterators and traits for converting trace data iterators into event iterators.
use super::{Detector, TracePoint};
use tracing::trace;

/// Applies a detector to a source iterator.
#[derive(Clone)]
pub(crate) struct EventIter<I, D>
where
    I: Iterator<Item = D::TracePointType>,
    D: Detector,
{
    /// The data to apply the detector to.
    source: I,
    /// The detector to apply.
    detector: D,
}

impl<I, D> Iterator for EventIter<I, D>
where
    I: Iterator<Item = D::TracePointType>,
    D: Detector,
{
    type Item = D::EventPointType;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.source.next() {
                Some(trace) => {
                    if let Some(event) = self.detector.signal(trace.get_time(), trace.clone_value())
                    {
                        trace!("Event found {event:?}");
                        return Some(event);
                    }
                }
                None => {
                    return self.detector.finish();
                }
            }
        }
    }
}

/// Should be implemented for any iterator which supports the `events`` method.
pub(crate) trait EventFilter<I, D>
where
    I: Iterator,
    I: Iterator<Item = D::TracePointType>,
    D: Detector,
{
    fn events(self, detector: D) -> EventIter<I, D>;
}

impl<I, D> EventFilter<I, D> for I
where
    I: Iterator,
    I: Iterator<Item = D::TracePointType>,
    D: Detector,
{
    /// Create an [EventIter] iterator, which applies a detector to a trace source as it is consumed.
    ///
    /// # Parameters
    /// - detector: A detector which is to be applied as the iterator is consumed.
    fn events(self, detector: D) -> EventIter<I, D> {
        EventIter {
            source: self,
            detector,
        }
    }
}
