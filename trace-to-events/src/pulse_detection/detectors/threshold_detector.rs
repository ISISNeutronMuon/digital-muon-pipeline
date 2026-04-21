//! This detector registers an event whenever the input stream passes a given threshold
//! value for a given time.
//!
//! The detector also implements a cool-down period to wait before another detection is registered.
use tracing::info;

use crate::pulse_detection::TracePoint;

use super::{Detector, EventData, Real};

/// The time-independnt data of the detector's event.
#[derive(Default, Debug, Clone, PartialEq)]
pub(crate) struct Data {
    pub(crate) pulse_height: Real,
}

impl EventData for Data {}

/// The current state of the detector.
#[derive(Default, Clone)]
enum DetectorState {
    /// The detector is waiting for the trace to exceed `begin_threshold`.
    #[default]
    Waiting,
    /// The trace has been over `begin_threshold` for less than `begin_duration`.
    Beginning {time_begun: Real },
    /// The trace has been over `begin_threshold` for at least `begin_duration`.
    Detected,
    /// The detector has just completed an event detection and is waiting to cool down, before being able to detect another.
    CoolingDown { time_ended: Real },
}

/// The triggering parameters of the threshold detector.
#[derive(Default, Debug, Clone)]
pub(crate) struct ThresholdDuration {
    /// The threshold the trace must exceed to trigger the detector.
    pub(crate) threshold: Real,
    /// How long the trace must be above the `threshold` to begin the detection.
    pub(crate) duration: i32,
    /// Minimum time between end of last pulse and detection of a new one.
    pub(crate) cool_off: i32,
}

/// This detector triggers an event when the trace exceeds the threshold.
#[derive(Default, Clone)]
pub(crate) struct ThresholdDetector {
    /// The detection parameters.
    trigger: ThresholdDuration,
    /// The current state of the detector.
    state: DetectorState,
    /// The state of a detection in progress.
    partial_event: Option<ThresholdEvent>,
}

impl ThresholdDetector {
    /// Creates a new detector with the given triggering parameters.
    /// # Parameters
    pub(crate) fn new(trigger: &ThresholdDuration) -> Self {
        Self {
            trigger: trigger.clone(),
            ..Default::default()
        }
    }

    fn complete_detection(&mut self, time: Real) {
        if self.trigger.cool_off.eq(&0) {
            self.state = DetectorState::Waiting;
        } else {
            self.state = DetectorState::CoolingDown { time_ended: time };
        }
    }

    fn update_state(&mut self, time: Real, value: Real) {
        match &self.state {
            DetectorState::Waiting => {
                if value > self.trigger.threshold {
                    self.partial_event = Some((time, Data { pulse_height: value }));
                    if self.trigger.duration.eq(&1) {
                        self.state = DetectorState::Detected;
                    } else {
                        self.state = DetectorState::Beginning { time_begun: time };
                    }
                }
            },
            DetectorState::Beginning { time_begun} => {
                if time == self.trigger.duration as Real + *time_begun {
                    // Potential detection has persisted for long enough to become a partial detection.
                    if value <= self.trigger.threshold {
                        // The detection is complete.
                        self.complete_detection(time);
                    } else {
                        // The detection is partial.
                        self.state = DetectorState::Detected;
                    }
                } else if value <= self.trigger.threshold {
                    self.partial_event = None;
                    self.state = DetectorState::Waiting;
                }
            },
            DetectorState::Detected => {
                if value <= self.trigger.threshold {
                    self.complete_detection(time);
                }
            },
            DetectorState::CoolingDown { time_ended } => {
                if time == *time_ended + self.trigger.cool_off as Real {
                    self.state = DetectorState::Waiting;
                }
            }
        }
    }

    /// If a partial event is in progress, take ownership of it as long as the state
    /// is `CoolingDown` or `Waiting`, otherwise return `None`.
    fn try_take_completed_event(&mut self) -> Option<ThresholdEvent> {
        match self.state {
            | DetectorState::CoolingDown { .. }
            | DetectorState::Waiting => self.partial_event.take(),
            _ => None,
        }
    }
}

/// The time-dependent event of the threshold detector.
pub(crate) type ThresholdEvent = (Real, Data);

impl Detector for ThresholdDetector {
    type TracePointType = (Real, Real);
    type EventOutputType = (Real, Data);

    fn signal(&mut self, 
        time: <Self::TracePointType as TracePoint>::Time,
        value: <Self::TracePointType as TracePoint>::Value
    ) -> Option<ThresholdEvent> {
        self.update_state(time, value);

        if let Some(partial_event) = self.partial_event.as_mut() {
            partial_event.1.pulse_height = partial_event.1.pulse_height.max(value);
        }
        self.try_take_completed_event()
    }

    fn finish(&mut self) -> Option<Self::EventOutputType> {
        self.partial_event.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{pulse_detection::{EventsIterable, Real}, test_data::{pyramid::INPUT, assert_iters_equal}};

    #[test]
    fn zero_data() {
        let data: [Real; 0] = [];
        let detector = ThresholdDetector::new(&ThresholdDuration {
            threshold: 2.0,
            cool_off: 0,
            duration: 2,
        });
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .events(detector);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_positive_threshold() {
        let data = [4, 3, 2, 5, 6, 1, 5, 7, 2, 4];
        let detector = ThresholdDetector::new(&ThresholdDuration {
            threshold: 2.0,
            cool_off: 0,
            duration: 2,
        });
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .events(detector);
        assert_eq!(iter.next(), Some((0.0, Data { pulse_height: 4.0 })));
        assert_eq!(iter.next(), Some((3.0, Data { pulse_height: 6.0 })));
        assert_eq!(iter.next(), Some((6.0, Data { pulse_height: 7.0 })));
        assert_eq!(iter.next(), Some((9.0, Data { pulse_height: 4.0 })));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_negative_threshold() {
        let data = [4, 3, 2, 5, 2, 1, 5, 7, 2, 2, 2, 4];
        let detector = ThresholdDetector::new(&ThresholdDuration {
            threshold: -2.5,
            cool_off: 0,
            duration: 2,
        });
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, -v as Real))
            .events(detector);
        assert_eq!(iter.next(), Some((4.0, Data { pulse_height: -1.0 })));
        assert_eq!(iter.next(), Some((8.0, Data { pulse_height: -2.0 })));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_zero_duration() {
        let data = [4, 3, 2, 5, 2, 1, 5, 7, 2, 2];
        let detector = ThresholdDetector::new(&ThresholdDuration {
            threshold: -2.5,
            cool_off: 0,
            duration: 0,
        });
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, -v as Real))
            .events(detector);
        assert_eq!(iter.next(), Some((8.0, Data { pulse_height: -2.0 })));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_cool_off() {
        // Without cool-off the detector triggers at the following points:
        //          .  .  x  .  x  x  .  .  x  x
        // With a 1 sample cool-off the detector triggers at the following points
        //          .  .  x  .  x  .  .  .  x  .
        // With a 2 sample cool-off the detector triggers at the following points
        //          .  .  x  .  .  x  .  .  x  .
        let data = [4, 3, 2, 5, 2, 1, 5, 7, 2, 2];
        let detector2 = ThresholdDetector::new(&ThresholdDuration {
            threshold: -2.5,
            cool_off: 2,
            duration: 1,
        });
        let mut iter = data
            .iter()
            .copied()
            .enumerate()
            .map(|(i, v)| (i as Real, -v as Real))
            .events(detector2);
        assert_eq!(iter.next(), Some((2.0, Data { pulse_height: -2.0 })));
        assert_eq!(iter.next(), Some((8.0, Data { pulse_height: -2.0 })));
        assert_eq!(iter.next(), None);

        let detector1 = ThresholdDetector::new(&ThresholdDuration {
            threshold: -2.5,
            cool_off: 1,
            duration: 1,
        });

        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, -v as Real))
            .events(detector1);
        assert_eq!(iter.next(), Some((2.0, Data { pulse_height: -2.0 })));
        assert_eq!(iter.next(), Some((5.0, Data { pulse_height: -1.0 })));
        assert_eq!(iter.next(), Some((8.0, Data { pulse_height: -2.0 })));
        assert_eq!(iter.next(), None);

        let detector0 = ThresholdDetector::new(&ThresholdDuration {
            threshold: -2.5,
            cool_off: 0,
            duration: 1,
        });

        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, -v as Real))
            .events(detector0);
        assert_eq!(iter.next(), Some((2.0, Data { pulse_height: -2.0 })));
        assert_eq!(iter.next(), Some((4.0, Data { pulse_height: -1.0 })));
        assert_eq!(iter.next(), Some((8.0, Data { pulse_height: -2.0 })));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_real_data() {
        let parameters = ThresholdDuration {
            threshold: 15.0,
            duration: 2,
            cool_off: 0,
        };
        let detector = ThresholdDetector::new(&parameters);
        let events = INPUT
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, 200.0*v as Real))
            .events(detector)
            .collect::<Vec<_>>();
        let expected_times = [0.0, 19.0, 125.0];
        let expected_heights = [26.456692913385837, 26.456692913385837, 28.03149606299212];
        assert_iters_equal(events.iter().map(|x|&x.0), expected_times.iter());
        assert_iters_equal(events.iter().map(|x|&x.1.pulse_height), expected_heights.iter());
    }
}
