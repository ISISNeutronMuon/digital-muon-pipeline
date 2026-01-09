//! Provides event iterators and traits for converting trace data iterators into event iterators.
use super::{Assembler, Detector, Pulse, TracePoint};
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

/// Provides method for converting an object into an [EventIter].
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


/// Applies an assembler to a source iterator of detector events.
#[derive(Clone)]
pub(crate) struct AssemblerIter<I, A>
where
    A: Assembler,
    I: Iterator<Item = <A::DetectorType as Detector>::EventPointType> + Clone,
{
    /// Source to apply the assembler to.
    source: I,
    /// Assembler to apply.
    assembler: A,
}

impl<I, A> Iterator for AssemblerIter<I, A>
where
    A: Assembler,
    I: Iterator<Item = <A::DetectorType as Detector>::EventPointType> + Clone,
{
    type Item = Pulse;

    fn next(&mut self) -> Option<Pulse> {
        for event in &mut self.source {
            let pulse = self.assembler.assemble_pulses(event);
            if pulse.is_some() {
                return pulse;
            }
        }
        None
    }
}

/// Provides method for converting an object into an [AssemblerIter].
pub(crate) trait AssembleFilter<I, A>
where
    A: Assembler,
    I: Iterator<Item = <A::DetectorType as Detector>::EventPointType> + Clone,
{
    fn assemble(self, assembler: A) -> AssemblerIter<I, A>;
}

impl<I, A> AssembleFilter<I, A> for I
where
    A: Assembler,
    I: Iterator<Item = <A::DetectorType as Detector>::EventPointType> + Clone,
{
    /// Create an [AssemblerIter] iterator, which applies an assembler to an event source as it is consumed.
    /// 
    /// # Parameters
    /// - assembler: An assembler which is to be applied as the iterator is consumed.
    fn assemble(self, assembler: A) -> AssemblerIter<I, A> {
        AssemblerIter {
            source: self,
            assembler,
        }
    }
}
