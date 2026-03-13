//! This detector registers an event whenever the input stream achieves a local minima.
use super::{Detector, EventData, Real};
use digital_muon_common::Time;

/// The time-independnt data of the detector's event.
pub(crate) type Data = (); //TraceArray<3, Real>;

impl EventData for Data {}

/// This detector triggers an event when the trace exceeds the threshold.
#[derive(Default, Clone)]
pub(crate) struct LocalArgMinDetector {
    default: Option<LocalArgMinEvent>,
    cache: CyclingCache,
}

/// The time-dependent event of the local minima detector.
pub(crate) type LocalArgMinEvent = (Time, Data);

#[derive(Default, Clone)]
struct CyclingCache {
    len: usize,
    back: Real,
    middle: Real,
}

impl CyclingCache {
    fn is_empty(&self) -> bool {
        self.len == 0
    }
    
    fn cycle_in_new(&mut self, front: Real) {
        self.back = self.middle;
        self.middle = front;
        if self.len != 2 {
            self.len += 1;
        }
    }

    fn test_for_minimum(&self, front: Real) -> bool {
        if self.len != 2 {
            return false;
        }
        self.back > self.middle && self.middle < front
    }

    fn cycle_in_new_and_test_for_minimum(&mut self, front: Real) -> bool {
        let result = self.test_for_minimum(front);
        self.cycle_in_new(front);
        result
    }
}

impl Detector for LocalArgMinDetector {
    type TracePointType = (Time, Real);
    type EventPointType = LocalArgMinEvent;

    fn signal(&mut self, time: Time, value: Real) -> Option<LocalArgMinEvent> {
        if self.cache.is_empty() {
            self.default = Some((time, ()));
        }

        let event = self.cache
            .cycle_in_new_and_test_for_minimum(value)
            .then(||(time - 1, ()));

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
            .map(|(i, v)| (i as Time, v as Real))
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
            .map(|(i, v)| (i as Time, v as Real))
            .events(detector);
        assert_eq!(iter.next(), Some((2, ())));
        assert_eq!(iter.next(), Some((5, ())));
        assert_eq!(iter.next(), Some((8, ())));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_data_without_local_mins() {
        let data = [4, 3, 3, 3, 2, 2, 2, 1, 0, 0];
        let detector = LocalArgMinDetector::default();
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Time, v as Real))
            .events(detector);
        assert_eq!(iter.next(), Some((0, ())));
        assert_eq!(iter.next(), None);
    }
}
