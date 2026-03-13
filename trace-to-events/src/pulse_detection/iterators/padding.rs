//! Provides event iterators and traits for converting trace data iterators into event iterators.
use crate::pulse_detection::Real;

/// Should be implemented for any iterator which supports the `events` method.
pub(crate) trait PaddingIterable:
    Iterator + Clone + ExactSizeIterator + DoubleEndedIterator
{
    fn pad_reflect(
        self,
        left_padding: usize,
        right_padding: usize,
    ) -> impl Iterator<Item = Self::Item> + Clone;
}

impl<I> PaddingIterable for I
where
    I: Iterator<Item = Real> + Clone + ExactSizeIterator + DoubleEndedIterator,
{
    /// Create an [EventIter] iterator, which applies a detector to a trace source as it is consumed.
    ///
    /// # Parameters
    /// - detector: A detector which is to be applied as the iterator is consumed.
    fn pad_reflect(
        self,
        left_padding: usize,
        right_padding: usize,
    ) -> impl Iterator<Item = Real> + Clone {
        assert!(left_padding < self.len());
        assert!(right_padding < self.len());
        let left_padding = self
            .clone()
            .take(left_padding)
            .rev();

        let right_padding = self
            .clone()
            .rev()
            .take(right_padding);

        left_padding.chain(self.clone()).chain(right_padding)
    }
}
/*
impl<I> PaddingIterable for I
where
    I: Iterator<Item = (Real, Real)> + Clone + ExactSizeIterator + DoubleEndedIterator,
{
    /// Create an [EventIter] iterator, which applies a detector to a trace source as it is consumed.
    ///
    /// # Parameters
    /// - detector: A detector which is to be applied as the iterator is consumed.
    fn pad_reflect(
        self,
        left_padding: usize,
        right_padding: usize,
    ) -> impl Iterator<Item = (Real, Real)> + Clone {
        assert!(left_padding < self.len());
        assert!(right_padding < self.len());
        let left_padding = self
            .clone()
            .map(|(t, v)| (-1.0 - t, v))
            .take(left_padding)
            .rev();

        let len = self.len() as Real;
        let right_padding = self
            .clone()
            .rev()
            .map(move |(t, v)| (2.0 * len - 1.0 - t, v))
            .take(right_padding);

        left_padding.chain(self.clone()).chain(right_padding)
    }
}
 */
#[cfg(test)]
mod tests {
    use super::*;

    const LEFT_PADDING: usize = 5;
    const RIGHT_PADDING: usize = 3;
    const Y: [i32; 7] = [354, 2346, 7756, 234, 3476547, 34575, -5634];

    #[test]
    fn test_padding() {
        let y_iter = Y.into_iter();
        let padded_vec = y_iter
            .clone()
            .map(|v| v as Real)
            .pad_reflect(LEFT_PADDING, RIGHT_PADDING)
            .collect::<Vec<_>>();

        assert_eq!(padded_vec.len(), Y.len() + LEFT_PADDING + RIGHT_PADDING);

        // Test Indices are Correct
        //for (i, &(t, _)) in padded_vec.iter().enumerate() {
        //    assert_eq!(i as Real - LEFT_PADDING as Real, t);
        //}

        // Test Left Padded Values are Correct
        for i in 0..LEFT_PADDING {
            assert_eq!(Y[i] as Real, padded_vec[LEFT_PADDING - i - 1]);
        }

        // Test Right Padded Values are Correct
        for i in 0..RIGHT_PADDING {
            assert_eq!(
                Y[Y.len() - RIGHT_PADDING + i] as Real,
                padded_vec[padded_vec.len() - i - 1]
            );
        }
    }
}
