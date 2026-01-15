//! Provides event iterators and traits for converting trace data iterators into event iterators.
use super::{Assembler, Detector, Pulse};

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
