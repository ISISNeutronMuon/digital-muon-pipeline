use super::{Detector, EventData, Real};
use crate::{
    parameters::PeakHeightMode,
    pulse_detection::{datatype::tracevalue::TraceArray, threshold_detector::ThresholdDuration},
};
use std::fmt::Display;

#[derive(Default, Debug, Clone, PartialEq)]
pub(crate) struct Data {
    pub(crate) base_height: Real,
    pub(crate) peak_height: Real,
}

impl Data {
    fn some_new_event(time: Real, base_height: Real, peak_height: Real) -> Option<(Real, Self)> {
        Some((
            time,
            Data {
                base_height,
                peak_height,
            },
        ))
    }
}

impl Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{}", self.base_height, self.peak_height)
    }
}

impl EventData for Data {}

#[derive(Clone)]
struct PartialEvent {
    /// The time at the pulse's initial detection.
    time_begun: Real,
    /// The height of the trace at the pulse's detection.
    base_height: Real,
    /// The time associated with the event, i.e. time of the rising edge.
    time_of_event: Real,
    /// The trace value at the time of the peak.
    peak_height: Real,
    /// The value/deriv pair at the time of maximum derivative.
    trace_array_at_max_deriv: TraceArray<2, Real>,
}

impl PartialEvent {
    fn update_max_derivative(&mut self, time: Real, value: TraceArray<2, Real>) {
        if self.trace_array_at_max_deriv[1] < value[1] {
            // Set update the max derivative if the current derivative is higher.
            self.trace_array_at_max_deriv = value;
            self.time_of_event = time;
        }
    }

    fn set_peak_height_to_last_value(&mut self, value: TraceArray<2, Real>) {
        self.peak_height = value[0] - value[1];
    }

    fn set_peak_height_to_max_value(&mut self, value: Real) {
        if self.peak_height < value {
            self.peak_height = value;
        }
    }

    fn into_some_event(self) -> Option<(Real, Data)> {
        Data::some_new_event(self.time_of_event, self.base_height, self.peak_height)
    }
}

#[derive(Default, Clone)]
pub(crate) struct DifferentialThresholdDetector {
    trigger: ThresholdDuration,
    peak_height_mode: PeakHeightMode,

    time_of_last_return: Option<Real>,
    partial_event: Option<PartialEvent>,
}

impl DifferentialThresholdDetector {
    pub(crate) fn new(trigger: &ThresholdDuration, peak_height_mode: PeakHeightMode) -> Self {
        Self {
            trigger: trigger.clone(),
            peak_height_mode,
            ..Default::default()
        }
    }

    fn init_new_partial_event(&mut self, time: Real, value: TraceArray<2, Real>) {
        self.partial_event = Some(PartialEvent {
            time_begun: time,
            time_of_event: time,
            trace_array_at_max_deriv: value,
            base_height: value[0] - value[1],
            peak_height: value[0],
        });
    }
}

pub(crate) type ThresholdEvent = (Real, Data);

impl Detector for DifferentialThresholdDetector {
    type TracePointType = (Real, TraceArray<2, Real>);
    type EventPointType = (Real, Data);

    fn signal(&mut self, time: Real, value: TraceArray<2, Real>) -> Option<ThresholdEvent> {
        match self.partial_event.as_mut() {
            Some(partial_event) => {
                // Update the max derivative depending on the method used.
                partial_event.update_max_derivative(time, value);
                match self.peak_height_mode {
                    PeakHeightMode::ValueAtEndTrigger => {
                        partial_event.set_peak_height_to_last_value(value)
                    }
                    PeakHeightMode::MaxValue => {
                        partial_event.set_peak_height_to_max_value(value[0])
                    }
                }

                // If the current differential is non-positive:
                if value[1] <= 0.0 {
                    let partial_event = self.partial_event.take();
                    partial_event.and_then(|partial_event| {
                        if time - partial_event.time_begun >= self.trigger.duration as Real {
                            self.time_of_last_return = Some(time);
                            partial_event.into_some_event()
                        } else {
                            None
                        }
                    })
                    })
                } else {
                    None
                }
            }
            None => {
                //  If we are under the threshold.

                // If the current value as over the threshold:
                //  If we are under the threshold.

                // If the current value as over the threshold:
                if value[1] > self.trigger.threshold {
                    // If we have a "time_of_last_return", then test if we have passed the cool-down time.
                    if let Some(time_of_last_return) = self.time_of_last_return {
                        if time - time_of_last_return >= self.trigger.cool_off as Real {
                            self.init_new_partial_event(time, value);
                        }
                    } else {
                        self.init_new_partial_event(time, value);
                    }
                }
                None
            }
        }
    }

    fn finish(&mut self) -> Option<Self::EventPointType> {
        self.partial_event
            .take()
            .and_then(|partial_event| partial_event.into_some_event());
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
            Default::default(),
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
            PeakHeightMode::ValueAtEndTrigger,
        );
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector);
        assert_eq!(iter.next(), Data::some_new_event(3.0, 2.0, 6.0));
        assert_eq!(iter.next(), Data::some_new_event(6.0, 1.0, 7.0));
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
            PeakHeightMode::MaxValue,
        );
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector);
        assert_eq!(iter.next(), Data::some_new_event(3.0, 2.0, 6.0));
        assert_eq!(iter.next(), Data::some_new_event(6.0, 1.0, 7.0));
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
            Default::default(),
        );
        let mut iter = data
            .iter()
            .copied()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector2);
        assert_eq!(iter.next(), Data::some_new_event(3.0, 2.0, 5.0));
        assert_eq!(iter.next(), Data::some_new_event(9.0, 2.0, 6.0));
        assert_eq!(iter.next(), Data::some_new_event(13.0, 8.0, 11.0));
        assert_eq!(iter.next(), None);

        let detector1 = DifferentialThresholdDetector::new(
            &ThresholdDuration {
                threshold: 2.5,
                cool_off: 2,
                duration: 1,
            },
            Default::default(),
        );

        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector1);
        assert_eq!(iter.next(), Data::some_new_event(3.0, 2.0, 5.0));
        assert_eq!(iter.next(), Data::some_new_event(6.0, 1.0, 7.0));
        assert_eq!(iter.next(), Data::some_new_event(11.0, 5.0, 8.0));
        assert_eq!(iter.next(), None);

        let detector0 = DifferentialThresholdDetector::new(
            &ThresholdDuration {
                threshold: 2.5,
                cool_off: 1,
                duration: 1,
            },
            Default::default(),
        );

        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector0);
        assert_eq!(iter.next(), Data::some_new_event(3.0, 2.0, 5.0));
        assert_eq!(iter.next(), Data::some_new_event(6.0, 1.0, 7.0));
        assert_eq!(iter.next(), Data::some_new_event(9.0, 2.0, 6.0));
        assert_eq!(iter.next(), Data::some_new_event(11.0, 5.0, 8.0));
        assert_eq!(iter.next(), Data::some_new_event(13.0, 8.0, 11.0));
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
            Default::default(),
        );
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector);
        assert_eq!(iter.next(), Data::some_new_event(17.0, 3.0, 112.0));
        assert_eq!(iter.next(), Data::some_new_event(50.0, 4.0, 113.0));
        assert_eq!(iter.next(), Data::some_new_event(77.0, 6.0, 111.0));
        assert_eq!(iter.next(), None);
    }
}
