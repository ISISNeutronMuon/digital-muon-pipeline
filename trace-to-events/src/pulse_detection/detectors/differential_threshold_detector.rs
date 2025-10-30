use super::{Detector, EventData, Real};
use crate::pulse_detection::{
    datatype::tracevalue::TraceArray, threshold_detector::ThresholdDuration,
};
use std::fmt::Display;

#[derive(Default, Debug, Clone, PartialEq)]
pub(crate) struct Data {
    pub(crate) pulse_height: Real,
}

impl Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pulse_height)
    }
}

impl EventData for Data {}

#[derive(Clone)]
struct PartialEvent {
    time_begun: Real,
    time_of_max: Real,
    max_derivative: TraceArray<2, Real>,
}

impl PartialEvent {
    fn update_max_derivative(&mut self, time: Real, value: TraceArray<2, Real>) {
        if self.max_derivative[1] < value[1] {
            // Set update the max derivative if the current derivative is higher.
            self.max_derivative = value;
            self.time_of_max = time;
        }
    }

    fn update_max_value(&mut self, time: Real, value: TraceArray<2, Real>) {
        if self.max_derivative[0] < value[0] {
            self.max_derivative = value;
            self.time_of_max = time;
        }
    }

    fn into_event(self, constant_multiple: Option<Real>) -> (Real, Data) {
        let pulse_height = constant_multiple
            .map(|mul| self.max_derivative[0] * mul)
            .unwrap_or(self.max_derivative[0]);
        (self.time_of_max, Data { pulse_height })
    }
}

#[derive(Default, Clone)]
pub(crate) struct DifferentialThresholdDetector {
    trigger: ThresholdDuration,
    /// If provided, the pulse height is the height of the rising edge, scaled by this value,
    /// otherwise, the pulse height is the maximum value of the trace, during the event detection.
    constant_multiple: Option<Real>,

    time_of_last_return: Option<Real>,
    partial_event: Option<PartialEvent>,
    //time_crossed: Option<Real>,
    //temp_time: Option<Real>,
    //max_derivative: TraceArray<2, Real>,
}

impl DifferentialThresholdDetector {
    pub(crate) fn new(trigger: &ThresholdDuration, constant_multiple: Option<Real>) -> Self {
        Self {
            trigger: trigger.clone(),
            constant_multiple,
            ..Default::default()
        }
    }
}

pub(crate) type ThresholdEvent = (Real, Data);

impl Detector for DifferentialThresholdDetector {
    type TracePointType = (Real, TraceArray<2, Real>);
    type EventPointType = (Real, Data);

    fn signal(&mut self, time: Real, value: TraceArray<2, Real>) -> Option<ThresholdEvent> {
        match self.partial_event.as_mut() {
            Some(partial_event) => {
                // If we are already over the threshold.

                // Update the max derivative depending on the method used (i.e. `constant multiple` or `max value`).
                if self.constant_multiple.is_some() {
                    partial_event.update_max_derivative(time, value);
                } else {
                    partial_event.update_max_value(time, value);
                }

                // If the current differential is non-positive:
                if value[1] <= 0.0 {
                    let partial_event = self.partial_event.take();
                    partial_event.and_then(|partial_event| {
                        if time - partial_event.time_begun >= self.trigger.duration as Real {
                            self.time_of_last_return = Some(time);
                            Some(partial_event.into_event(self.constant_multiple))
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            }
            None => {
                //  If we are under the threshold.

                // If the current value as over the threshold:
                if value[1] > self.trigger.threshold {
                    // If we have a "time_of_last_return", then test if we have passed the cool-down time.
                    if let Some(time_of_last_return) = self.time_of_last_return {
                        if time - time_of_last_return >= self.trigger.cool_off as Real {
                            self.partial_event = Some(PartialEvent { time_begun: time, time_of_max: time, max_derivative: value });
                        }
                    } else {
                        self.partial_event = Some(PartialEvent { time_begun: time, time_of_max: time, max_derivative: value });
                    }
                }
                None
            }
        }
    }

    fn finish(&mut self) -> Option<Self::EventPointType> {
        self.partial_event.take().map(|partial_event| {
            partial_event.into_event(self.constant_multiple)
        });
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse_detection::{EventFilter, Real, WindowFilter, window::FiniteDifferences};
    use digital_muon_common::Intensity;

    #[test]
    fn zero_data() {
        let data: [Real; 0] = [];
        let detector = DifferentialThresholdDetector::new(
            &ThresholdDuration {
                threshold: 2.0,
                cool_off: 0,
                duration: 2,
            },
            Some(2.0),
        );
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_positive_threshold() {
        let data = [4, 3, 2, 5, 6, 1, 5, 7, 2, 4];
        let detector = DifferentialThresholdDetector::new(
            &ThresholdDuration {
                threshold: 2.0,
                cool_off: 0,
                duration: 2,
            },
            Some(2.0),
        );
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector);
        assert_eq!(iter.next(), Some((3.0, Data { pulse_height: 10.0 })));
        assert_eq!(iter.next(), Some((6.0, Data { pulse_height: 10.0 })));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_positive_threshold_no_constant_multiple() {
        let data = [4, 3, 2, 5, 6, 1, 5, 7, 2, 4];
        let detector = DifferentialThresholdDetector::new(
            &ThresholdDuration {
                threshold: 2.0,
                cool_off: 0,
                duration: 2,
            },
            None,
        );
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector);
        assert_eq!(iter.next(), Some((4.0, Data { pulse_height: 6.0 })));
        assert_eq!(iter.next(), Some((7.0, Data { pulse_height: 7.0 })));
        assert_eq!(iter.next(), None);
    }
/*
    #[test]
    fn test_zero_duration() {
        let data = [4, 3, 2, 5, 2, 1, 5, 7, 2, 2];
        let detector = DifferentialThresholdDetector::new(
            &ThresholdDuration {
                threshold: -2.5,
                cool_off: 0,
                duration: 0,
            },
            Some(2.0),
        );
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, -v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector);
        assert_eq!(iter.next(), None);
    }
 */
    #[test]
    fn test_cool_off() {
        // With a 1 sample cool-off the detector triggers at the following points
        //          .  .  .  x  .  .  x  .  .  x  .  x  .  x
        // With a 2 sample cool-off the detector triggers at the following points
        //          .  .  .  x  .  .  x  .  .  .  .  x  .  .
        // With a 3 sample cool-off the detector triggers at the following points
        //          .  .  .  x  .  .  .  .  .  x  .  .  .  x
        let data = [4, 3, 2, 5, 2, 1, 5, 7, 2, 6, 5, 8, 8, 11, 0];
        let detector2 = DifferentialThresholdDetector::new(
            &ThresholdDuration {
                threshold: 2.5,
                cool_off: 3,
                duration: 1,
            },
            Some(2.0),
        );
        let mut iter = data
            .iter()
            .copied()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector2);
        assert_eq!(iter.next(), Some((3.0, Data { pulse_height: 10.0 })));
        assert_eq!(iter.next(), Some((9.0, Data { pulse_height: 12.0 })));
        assert_eq!(iter.next(), Some((13.0, Data { pulse_height: 22.0 })));
        assert_eq!(iter.next(), None);

        let detector1 = DifferentialThresholdDetector::new(
            &ThresholdDuration {
                threshold: 2.5,
                cool_off: 2,
                duration: 1,
            },
            Some(2.0),
        );

        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector1);
        assert_eq!(iter.next(), Some((3.0, Data { pulse_height: 10.0 })));
        assert_eq!(iter.next(), Some((6.0, Data { pulse_height: 10.0 })));
        assert_eq!(iter.next(), Some((11.0, Data { pulse_height: 16.0 })));
        assert_eq!(iter.next(), None);

        let detector0 = DifferentialThresholdDetector::new(
            &ThresholdDuration {
                threshold: 2.5,
                cool_off: 1,
                duration: 1,
            },
            Some(2.0),
        );

        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector0);
        assert_eq!(iter.next(), Some((3.0, Data { pulse_height: 10.0 })));
        assert_eq!(iter.next(), Some((6.0, Data { pulse_height: 10.0 })));
        assert_eq!(iter.next(), Some((9.0, Data { pulse_height: 12.0 })));
        assert_eq!(iter.next(), Some((11.0, Data { pulse_height: 16.0 })));
        assert_eq!(iter.next(), Some((13.0, Data { pulse_height: 22.0 })));
        assert_eq!(iter.next(), None);
    }

    fn b2bexp(
        x: Real,
        ampl: Real,
        spread: Real,
        x0: Real,
        rising: Real,
        falling: Real,
    ) -> Intensity {
        let normalising_factor = ampl * 0.5 * (rising * falling) / (rising + falling);
        let rising_spread = rising * spread.powi(2);
        let falling_spread = falling * spread.powi(2);
        let x_shift = x - x0;
        let rising_exp = Real::exp(rising * 0.5 * (rising_spread + 2.0 * x_shift));
        let rising_erfc = libm::erfc((rising_spread + x_shift) / (Real::sqrt(2.0) * spread));
        let falling_exp = Real::exp(falling * 0.5 * (falling_spread - 2.0 * x_shift));
        let falling_erfc = libm::erfc((falling_spread - x_shift) / (Real::sqrt(2.0) * spread));
        (normalising_factor * (rising_exp * rising_erfc + falling_exp * falling_erfc)) as Intensity
    }

    #[test]
    fn test_b2bexp() {
        let range = 0..100;
        let data = range
            .clone()
            .map(|x| {
                b2bexp(x as Real, 1000.0, 3.5, 20.0, 3.5, 2.25)
                    + b2bexp(x as Real, 1000.0, 3.5, 54.0, 4.5, 5.5)
                    + b2bexp(x as Real, 1000.0, 3.5, 81.0, 1.5, 3.25)
            })
            .collect::<Vec<_>>();

        let detector = DifferentialThresholdDetector::new(
            &ThresholdDuration {
                threshold: 3.0,
                cool_off: 0,
                duration: 1,
            },
            Some(2.0),
        );
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector);
        let result = Some((
            17.0,
            Data {
                pulse_height: 150.0,
            },
        ));
        assert_eq!(iter.next(), result);
        let result = Some((
            50.0,
            Data {
                pulse_height: 120.0,
            },
        ));
        assert_eq!(iter.next(), result);
        let result = Some((
            77.0,
            Data {
                pulse_height: 132.0,
            },
        ));
        assert_eq!(iter.next(), result);
        assert_eq!(iter.next(), None);
    }
}
