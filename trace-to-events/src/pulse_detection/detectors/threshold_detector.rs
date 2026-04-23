//! This detector registers an event whenever the input stream passes a given threshold
//! value for a given time.
//!
//! The detector also implements a cool-down period to wait before another detection is registered.

use super::{Detector, EventData, Real};
use crate::pulse_detection::TracePoint;

/// Helper Type for the time type used for by the detector.
type DetectorTime = <<ThresholdDetector as Detector>::TracePointType as TracePoint>::Time;
/// Helper Type for the value type used for by the detector.
type DetectorValue = <<ThresholdDetector as Detector>::TracePointType as TracePoint>::Value;

/// The time-independnt data of the detector's event.
#[derive(Default, Debug, Clone, PartialEq)]
pub(crate) struct Data {
    pub(crate) pulse_height: DetectorValue,
}

impl EventData for Data {}

/// The current state of the detector.
#[derive(Default, Clone)]
enum DetectorState {
    /// The detector is waiting for the trace to exceed `begin_threshold`.
    #[default]
    Waiting,
    /// The trace has been over `begin_threshold` for less than `begin_duration`.
    Beginning { time_begun: DetectorTime },
    /// The trace has been over `begin_threshold` for at least `begin_duration`.
    Detected,
    /// The detector has just completed an event detection and is waiting to cool down, before being able to detect another.
    CoolingDown { time_ended: DetectorTime },
}

/// The triggering parameters of the threshold detector.
#[derive(Default, Debug, Clone)]
pub(crate) struct ThresholdDetectorParameters {
    /// The threshold the trace must exceed to trigger the detector.
    pub(crate) threshold: DetectorValue,
    /// How long the trace must be above the `threshold` to begin the detection.
    pub(crate) duration: usize,
    /// Minimum time between end of last pulse and detection of a new one.
    pub(crate) cool_off: usize,
}

/// This detector triggers an event when the trace exceeds the threshold.
#[derive(Default, Clone)]
pub(crate) struct ThresholdDetector {
    /// The detection parameters.
    parameters: ThresholdDetectorParameters,
    /// The current state of the detector.
    state: DetectorState,
    /// The state of a detection in progress.
    partial_event: Option<ThresholdEvent>,
}

impl ThresholdDetector {
    /// Creates a new detector with the given triggering parameters.
    /// # Parameters
    pub(crate) fn new(parameters: &ThresholdDetectorParameters) -> Self {
        Self {
            parameters: parameters.clone(),
            ..Default::default()
        }
    }

    fn complete_detection(&mut self, time: DetectorTime) {
        if self.parameters.cool_off.eq(&0) {
            self.state = DetectorState::Waiting;
        } else {
            self.state = DetectorState::CoolingDown { time_ended: time };
        }
    }

    fn update_state(&mut self, time: DetectorTime, value: DetectorValue) {
        match &self.state {
            DetectorState::Waiting => {
                if value > self.parameters.threshold {
                    self.partial_event = Some((
                        time,
                        Data {
                            pulse_height: value,
                        },
                    ));
                    if self.parameters.duration.eq(&1) {
                        self.state = DetectorState::Detected;
                    } else {
                        self.state = DetectorState::Beginning { time_begun: time };
                    }
                }
            }
            DetectorState::Beginning { time_begun } => {
                if time == self.parameters.duration as DetectorTime + *time_begun {
                    // Potential detection has persisted for long enough to become a partial detection.
                    if value <= self.parameters.threshold {
                        // The detection is complete.
                        self.complete_detection(time);
                    } else {
                        // The detection is partial.
                        self.state = DetectorState::Detected;
                    }
                } else if value <= self.parameters.threshold {
                    self.partial_event = None;
                    self.state = DetectorState::Waiting;
                }
            }
            DetectorState::Detected => {
                if value <= self.parameters.threshold {
                    self.complete_detection(time);
                }
            }
            DetectorState::CoolingDown { time_ended } => {
                if time == *time_ended + self.parameters.cool_off as DetectorTime {
                    self.state = DetectorState::Waiting;
                }
            }
        }
    }

    /// If a partial event is in progress, take ownership of it as long as the state
    /// is `CoolingDown` or `Waiting`, otherwise return `None`.
    fn try_take_completed_event(&mut self) -> Option<ThresholdEvent> {
        match self.state {
            DetectorState::CoolingDown { .. } | DetectorState::Waiting => self.partial_event.take(),
            _ => None,
        }
    }
}

/// The time-dependent event of the threshold detector.
pub(crate) type ThresholdEvent = (DetectorTime, Data);

impl Detector for ThresholdDetector {
    type TracePointType = (usize, Real);
    type EventPointType = ThresholdEvent;

    fn signal(
        &mut self,
        time: <Self::TracePointType as TracePoint>::Time,
        value: <Self::TracePointType as TracePoint>::Value,
    ) -> Option<ThresholdEvent> {
        self.update_state(time, value);

        if let Some(partial_event) = self.partial_event.as_mut() {
            partial_event.1.pulse_height = partial_event.1.pulse_height.max(value);
        }
        self.try_take_completed_event()
    }

    fn finish(&mut self) -> Option<Self::EventPointType> {
        self.partial_event.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pulse_detection::{EventsIterable, Real},
        test_data::{assert_iters_approx_equal, assert_iters_equal, pyramid::INPUT},
    };

    #[test]
    fn zero_data() {
        let data: [Real; 0] = [];
        let detector = ThresholdDetector::new(&ThresholdDetectorParameters {
            threshold: 2.0,
            cool_off: 0,
            duration: 2,
        });
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as DetectorTime, v as DetectorValue))
            .events(detector);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_positive_threshold() {
        let data = [4, 3, 2, 5, 6, 1, 5, 7, 2, 4];
        let detector = ThresholdDetector::new(&ThresholdDetectorParameters {
            threshold: 2.0,
            cool_off: 0,
            duration: 2,
        });
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as DetectorTime, v as DetectorValue))
            .events(detector);
        assert_eq!(
            iter.next(),
            Some((
                0 as DetectorTime,
                Data {
                    pulse_height: 4 as DetectorValue
                }
            ))
        );
        assert_eq!(
            iter.next(),
            Some((
                3 as DetectorTime,
                Data {
                    pulse_height: 6 as DetectorValue
                }
            ))
        );
        assert_eq!(
            iter.next(),
            Some((
                6 as DetectorTime,
                Data {
                    pulse_height: 7 as DetectorValue
                }
            ))
        );
        assert_eq!(
            iter.next(),
            Some((
                9 as DetectorTime,
                Data {
                    pulse_height: 4 as DetectorValue
                }
            ))
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_negative_threshold() {
        let data = [4, 3, 2, 5, 2, 1, 5, 7, 2, 2, 2, 4];
        let detector = ThresholdDetector::new(&ThresholdDetectorParameters {
            threshold: -2.5,
            cool_off: 0,
            duration: 2,
        });
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as DetectorTime, -v as DetectorValue))
            .events(detector);
        assert_eq!(
            iter.next(),
            Some((4 as DetectorTime, Data { pulse_height: -1.0 }))
        );
        assert_eq!(
            iter.next(),
            Some((8 as DetectorTime, Data { pulse_height: -2.0 }))
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_zero_duration() {
        let data = [4, 3, 2, 5, 2, 1, 5, 7, 2, 2];
        let detector = ThresholdDetector::new(&ThresholdDetectorParameters {
            threshold: -2.5,
            cool_off: 0,
            duration: 0,
        });
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as DetectorTime, -v as DetectorValue))
            .events(detector);
        assert_eq!(
            iter.next(),
            Some((8 as DetectorTime, Data { pulse_height: -2.0 }))
        );
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
        let detector2 = ThresholdDetector::new(&ThresholdDetectorParameters {
            threshold: -2.5,
            cool_off: 2,
            duration: 1,
        });
        let mut iter = data
            .iter()
            .copied()
            .enumerate()
            .map(|(i, v)| (i as DetectorTime, -v as DetectorValue))
            .events(detector2);
        assert_eq!(
            iter.next(),
            Some((2 as DetectorTime, Data { pulse_height: -2.0 }))
        );
        assert_eq!(
            iter.next(),
            Some((8 as DetectorTime, Data { pulse_height: -2.0 }))
        );
        assert_eq!(iter.next(), None);

        let detector1 = ThresholdDetector::new(&ThresholdDetectorParameters {
            threshold: -2.5,
            cool_off: 1,
            duration: 1,
        });

        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as DetectorTime, -v as DetectorValue))
            .events(detector1);
        assert_eq!(
            iter.next(),
            Some((2 as DetectorTime, Data { pulse_height: -2.0 }))
        );
        assert_eq!(
            iter.next(),
            Some((5 as DetectorTime, Data { pulse_height: -1.0 }))
        );
        assert_eq!(
            iter.next(),
            Some((8 as DetectorTime, Data { pulse_height: -2.0 }))
        );
        assert_eq!(iter.next(), None);

        let detector0 = ThresholdDetector::new(&ThresholdDetectorParameters {
            threshold: -2.5,
            cool_off: 0,
            duration: 1,
        });

        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as DetectorTime, -v as DetectorValue))
            .events(detector0);
        assert_eq!(
            iter.next(),
            Some((2 as DetectorTime, Data { pulse_height: -2.0 }))
        );
        assert_eq!(
            iter.next(),
            Some((4 as DetectorTime, Data { pulse_height: -1.0 }))
        );
        assert_eq!(
            iter.next(),
            Some((8 as DetectorTime, Data { pulse_height: -2.0 }))
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_real_data() {
        let parameters = ThresholdDetectorParameters {
            threshold: 15.0,
            duration: 2,
            cool_off: 0,
        };
        let detector = ThresholdDetector::new(&parameters);
        let events = INPUT
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as DetectorTime, 200.0 * v as DetectorValue))
            .events(detector)
            .collect::<Vec<_>>();
        let expected_times = [0 as DetectorTime, 19 as DetectorTime, 125 as DetectorTime];
        let expected_heights = [
            26.456692913385837 as DetectorValue,
            26.456692913385837 as DetectorValue,
            28.03149606299212 as DetectorValue,
        ];
        assert_iters_equal(events.iter().map(|x| &x.0), expected_times.iter());
        assert_iters_approx_equal(
            events.iter().map(|x| &x.1.pulse_height),
            expected_heights.iter(),
        );
    }
}
