//! This detector registers an event whenever the input stream achieves a local minima.
use super::{Detector, Real};
use crate::pulse_detection::EventPoint;

impl EventPoint for usize {
    type TimeType = usize;
    type EventType = usize;
}

/// The time-dependent event of the local minima detector.
pub(crate) type LocalArgMinEvent = usize;

/// A FIFO buffer with an effective size of 3 (through only 2 values are stored at a time).
#[derive(Default, Clone)]
struct FifoBuffer {
    /// Size of the buffer, can be 0, 1, or 2.
    len: usize,
    /// Oldest value stored in the buffer.
    oldest: Real,
    /// Second oldest value stored in the buffer.
    middle: Real,
}

impl FifoBuffer {
    /// Tests whether any values have been pushed yet.
    fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Push the newest value into the buffer and displace the oldest value.
    fn push(&mut self, newest: Real) {
        self.oldest = self.middle;
        self.middle = newest;
        if self.len != 2 {
            self.len += 1;
        }
    }

    /// Tests whether the middle value is a minimum.
    /// This requires the user to provide the `newest` value.
    fn test_middle_for_minimum(&self, newest: Real) -> bool {
        if self.len != 2 {
            return false;
        }
        self.oldest > self.middle && self.middle < newest
    }

    /// Tests whether the middle value is a minimum and push the newest value into the buffer.
    /// This requires the user to provide the `newest` value.
    fn push_and_test_middle_for_minimum(&mut self, newest: Real) -> bool {
        let result = self.test_middle_for_minimum(newest);
        self.push(newest);
        result
    }
}

/// This detector triggers an event when the trace exceeds the threshold.
#[derive(Default, Clone)]
pub(crate) struct LocalArgMinDetector {
    /// Value to return if no local minima are found.
    default: Option<LocalArgMinEvent>,
    /// Buffer for storing trace values.
    cache: FifoBuffer,
}

impl Detector for LocalArgMinDetector {
    type TracePointType = (usize, Real);
    type EventPointType = LocalArgMinEvent;

    fn signal(&mut self, time: usize, value: Real) -> Option<LocalArgMinEvent> {
        if self.cache.is_empty() {
            self.default = Some(time);
        }

        let event = self
            .cache
            .push_and_test_middle_for_minimum(value)
            .then(|| time - 1);

        if self.default.is_some() && event.is_some() {
            self.default = None
        }
        event
    }

    fn finish(&mut self) -> Option<Self::EventPointType> {
        self.default.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse_detection::{EventsIterable, Real};

    #[test]
    fn zero_data() {
        let data: [Real; 0] = [];
        let detector = LocalArgMinDetector::default();
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i, v as Real))
            .events(detector);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_data_with_local_mins() {
        let data = [4, 3, 2, 5, 6, 1, 5, 7, 2, 4];
        let detector = LocalArgMinDetector::default();
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i, v as Real))
            .events(detector);
        assert_eq!(iter.next(), Some(2));
        assert_eq!(iter.next(), Some(5));
        assert_eq!(iter.next(), Some(8));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_data_without_local_mins() {
        let data = [4, 3, 3, 3, 2, 2, 2, 1, 0, 0];
        let detector = LocalArgMinDetector::default();
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i, v as Real))
            .events(detector);
        assert_eq!(iter.next(), Some(0));
        assert_eq!(iter.next(), None);
    }
}
