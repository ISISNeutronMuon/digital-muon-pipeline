//! This detector registers an event whenever the derivative of the input stream passes a given threshold
//! value for a given time.
//! 
//! The detector also implements a cool-down period to wait before another detection is registered.
use super::{Detector, EventData, Real};
use crate::{parameters::PeakHeightMode, pulse_detection::datatype::tracevalue::TraceArray};
use num::Zero;
use std::fmt::Display;

#[derive(Default, Debug, Clone)]
pub(crate) struct DifferentialThresholdParameters {
    /// The differential threshold the trace must exceed to trigger the detector.
    pub(crate) begin_threshold: Real,
    /// How long the trace derivative must be above the `begin_threshold` to begin the detection.
    pub(crate) begin_duration: Real,
    /// The differential threshold the trace must fall below to complete a detection.
    pub(crate) end_threshold: Real,
    /// How long the trace derivative must be below the `end_threshold` to complete the detection.
    pub(crate) end_duration: Real,
    /// Minimum time between end of last pulse and detection of a new one.
    pub(crate) cool_off: Real,
}

/// The time-independent parameters of the recorded pulse.
#[derive(Default, Debug, Clone, PartialEq)]
pub(crate) struct Data {
    /// The trace value at the base of the pulse.
    pub(crate) base_height: Real,
    /// The trace value at the peak of the pulse.
    pub(crate) peak_height: Real,
}

impl Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{}", self.base_height, self.peak_height)
    }
}

impl EventData for Data {}

/// The current state of the detector.
#[derive(Default, Clone)]
enum DetectorState {
    /// The detector is waiting for the trace to exceed `begin_threshold`.
    #[default]
    Waiting,
    /// The trace has been over `begin_threshold` for less than least `begin_duration`.
    Beginning { time_begun: Real },
    /// The trace has been over `begin_threshold` for at least `begin_duration`.
    Detected,
    /// The trace has been below `end_threshold` for at less than `end_duration`, having previously been in the `Detected` state..
    Ending { time_ended: Real },
    /// The detector has just completed an event detection and is waiting to cool down, before being able to detect another.
    CoolingDown { time_ended: Real },
}

/// (Time, Data) pair defining a pulse detection event.
pub(crate) type ThresholdEvent = (Real, Data);

/// Represents an event in the process of being detected.
#[derive(Clone)]
struct PartialEvent {
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
    /// Create and initialise new partial event from the inital trace values.
    fn new(time: Real, value: TraceArray<2, Real>) -> Self {
        Self {
            time_of_event: time,
            trace_array_at_max_deriv: value,
            base_height: value[0] - value[1],
            peak_height: value[0],
        }
    }

    /// Applies new trace data to the current event in progress.
    fn update(&mut self, peak_height_mode: PeakHeightMode, time: Real, value: TraceArray<2, Real>) {
        // Updates the max derivative if the current derivative is higher.
        if self.trace_array_at_max_deriv[1] < value[1] {
            self.trace_array_at_max_deriv = value;
            self.time_of_event = time;
        }

        // Updates the peak height, based on the mode specified.
        self.peak_height = match peak_height_mode {
            PeakHeightMode::ValueAtEndTrigger => value[0] - value[1],
            PeakHeightMode::MaxValue => Real::max(self.peak_height, value[0]),
        };
    }

    /// Convert partial event into a `ThresholdEvent`.
    fn into_event(self) -> ThresholdEvent {
        (
            self.time_of_event,
            Data {
                base_height: self.base_height,
                peak_height: self.peak_height,
            },
        )
    }
}

/// Detects pulses in a trace by analysing the differential of the trace.
#[derive(Default, Clone)]
pub(crate) struct DifferentialThresholdDetector {
    /// The detection parameters.
    parameters: DifferentialThresholdParameters,
    /// Determines how peak heights are calculated. This does not affect the number, or time of detections.
    peak_height_mode: PeakHeightMode,

    /// The current state of the detector.
    state: DetectorState,
    /// The state of a detection in progress.
    partial_event: Option<PartialEvent>,
}

impl DifferentialThresholdDetector {
    /// Create new detector.
    pub(crate) fn new(
        parameters: &DifferentialThresholdParameters,
        peak_height_mode: PeakHeightMode,
    ) -> Self {
        Self {
            parameters: parameters.clone(),
            peak_height_mode,
            ..Default::default()
        }
    }

    /// Modifies the detector state based on the current state, and outputs an event if appropriate.
    ///
    /// # Possible State Changes
    /// The next state can only be one of the following, depending on the current state
    /// |Current State| Next State |
    /// |--|--|
    /// |`Waiting`|`Beginning` or `Detected`|
    /// |`Beginning`|`Waiting` or `Detected`|
    /// |`Detected`|`Ending`, `CoolingDown` or `Waiting`|
    /// |`Ending`|`Detected` or `CoolingDown`|
    /// |`CoolingDown`|`Waiting`|
    ///
    /// # Allowed States
    /// The following states will only ever occur if the following conditions are true.
    /// |State|Only If|
    /// |--|--|
    /// |`Beginning`|`self.parameters.begin_duration` is nonzero|
    /// |`Ending`|`self.parameters.end_duration` is nonzero|
    /// |`CoolingDown`|`self.parameters.cooloff` is nonzero|
    fn update_state(&mut self, time: Real, value: TraceArray<2, Real>) {
        match self.state {
            DetectorState::Waiting => {
                if value[1] >= self.parameters.begin_threshold {
                    self.partial_event = Some(PartialEvent::new(time, value));
                    if self.parameters.begin_duration.is_zero() {
                        self.state = DetectorState::Detected;
                    } else {
                        self.state = DetectorState::Beginning { time_begun: time };
                    }
                }
            }
            DetectorState::Beginning { time_begun } => {
                if time >= time_begun + self.parameters.begin_duration {
                    self.state = DetectorState::Detected;
                } else if value[1] < self.parameters.begin_threshold {
                    self.partial_event = None;
                    self.state = DetectorState::Waiting;
                }
            }
            DetectorState::Detected => {
                if value[1] <= self.parameters.end_threshold {
                    if self.parameters.end_duration.is_zero() {
                        if self.parameters.cool_off.is_zero() {
                            self.state = DetectorState::Waiting;
                        } else {
                            self.state = DetectorState::CoolingDown { time_ended: time };
                        }
                    } else {
                        self.state = DetectorState::Ending { time_ended: time };
                    }
                }
            }
            DetectorState::Ending { time_ended } => {
                if time >= time_ended + self.parameters.end_duration {
                    if self.parameters.cool_off.is_zero() {
                        self.state = DetectorState::Waiting;
                    } else {
                        self.state = DetectorState::CoolingDown { time_ended: time };
                    }
                } else if value[1] > self.parameters.end_threshold {
                    self.state = DetectorState::Detected;
                }
            }
            DetectorState::CoolingDown { time_ended } => {
                if time >= time_ended + self.parameters.cool_off {
                    self.state = DetectorState::Waiting;
                }
            }
        }
    }

    /// If a partial event is in progress, take ownership of it as long as the sate
    /// is `Ending`, `CoolingDown` or `Waiting`, otherwise return `None`.
    fn try_take_completed_event(&mut self) -> Option<PartialEvent> {
        match self.state {
            DetectorState::Ending { .. }
            | DetectorState::CoolingDown { .. }
            | DetectorState::Waiting => self.partial_event.take(),
            _ => None,
        }
    }
}

impl Detector for DifferentialThresholdDetector {
    type TracePointType = (Real, TraceArray<2, Real>);
    type EventPointType = ThresholdEvent;

    fn signal(&mut self, time: Real, value: TraceArray<2, Real>) -> Option<ThresholdEvent> {
        self.update_state(time, value);

        if let Some(mut event) = self.try_take_completed_event() {
            event.update(self.peak_height_mode.clone(), time, value);
            Some(event.into_event())
        } else {
            if let Some(partial_event) = self.partial_event.as_mut() {
                partial_event.update(self.peak_height_mode.clone(), time, value);
            }
            None
        }
    }

    fn finish(&mut self) -> Option<Self::EventPointType> {
        self.partial_event
            .take()
            .map(|partial_event| partial_event.into_event());
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse_detection::{EventFilter, Real, WindowFilter, window::FiniteDifferences};
    use digital_muon_common::Intensity;

    fn pipeline(
        data: &[Intensity],
        detector: DifferentialThresholdDetector,
    ) -> impl Iterator<Item = (f64, Data)> {
        data.iter()
            .copied()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector)
    }

    fn some_new_event(time: Real, base_height: Real, peak_height: Real) -> Option<ThresholdEvent> {
        Some((
            time,
            Data {
                base_height,
                peak_height,
            },
        ))
    }

    #[test]
    fn zero_data() {
        let data: [Intensity; 0] = [];
        let detector = DifferentialThresholdDetector::new(
            &DifferentialThresholdParameters {
                begin_threshold: 2.0,
                end_threshold: 0.0,
                begin_duration: 2.0,
                ..Default::default()
            },
            Default::default(),
        );
        let mut iter = pipeline(&data, detector);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_positive_threshold() {
        let data = [4, 3, 2, 5, 6, 1, 5, 7, 6, 4, 5];
        let parameters = DifferentialThresholdParameters {
            begin_threshold: 3.0,
            end_threshold: -2.0,
            ..Default::default()
        };
        let detector =
            DifferentialThresholdDetector::new(&parameters, PeakHeightMode::ValueAtEndTrigger);
        let mut iter = pipeline(&data, detector);
        assert_eq!(iter.next(), some_new_event(3.0, 2.0, 6.0));
        assert_eq!(iter.next(), some_new_event(6.0, 1.0, 6.0));
        assert_eq!(iter.next(), None);

        let detector = DifferentialThresholdDetector::new(&parameters, PeakHeightMode::MaxValue);
        let mut iter = pipeline(&data, detector);
        assert_eq!(iter.next(), some_new_event(3.0, 2.0, 6.0));
        assert_eq!(iter.next(), some_new_event(6.0, 1.0, 7.0));
        assert_eq!(iter.next(), None);
    }

    mod begin_duration {
        use super::*;
        const DATA: [Intensity; 17] = [4, 3, 2, 5, 8, 12, 2, 1, 5, 7, 2, 6, 5, 8, 8, 11, 0];

        #[test]
        fn test_duration_3() {
            let parameters = DifferentialThresholdParameters {
                begin_threshold: 2.5,
                end_threshold: 0.0,
                begin_duration: 3.0,
                ..Default::default()
            };
            let detector = DifferentialThresholdDetector::new(&parameters, Default::default());
            let mut iter = pipeline(&DATA, detector);

            assert_eq!(iter.next(), some_new_event(5.0, 2.0, 12.0));
            assert_eq!(iter.next(), None);
        }

        #[test]
        fn test_duration_2() {
            let parameters = DifferentialThresholdParameters {
                begin_threshold: 2.5,
                end_threshold: 0.0,
                begin_duration: 2.0,
                ..Default::default()
            };
            let detector = DifferentialThresholdDetector::new(&parameters, Default::default());
            let mut iter = pipeline(&DATA, detector);

            assert_eq!(iter.next(), some_new_event(5.0, 2.0, 12.0));
            assert_eq!(iter.next(), None);
        }

        #[test]
        fn test_duration_1() {
            let parameters = DifferentialThresholdParameters {
                begin_threshold: 2.5,
                end_threshold: 0.0,
                begin_duration: 1.0,
                ..Default::default()
            };
            let detector = DifferentialThresholdDetector::new(&parameters, Default::default());

            let mut iter = pipeline(&DATA, detector);
            assert_eq!(iter.next(), some_new_event(5.0, 2.0, 12.0));
            assert_eq!(iter.next(), some_new_event(8.0, 1.0, 7.0));
            assert_eq!(iter.next(), some_new_event(11.0, 2.0, 8.0));
            assert_eq!(iter.next(), None);
        }

        #[test]
        fn test_duration_0() {
            let parameters = DifferentialThresholdParameters {
                begin_threshold: 2.5,
                end_threshold: 0.0,
                ..Default::default()
            };
            let detector = DifferentialThresholdDetector::new(&parameters, Default::default());

            let mut iter = pipeline(&DATA, detector);
            assert_eq!(iter.next(), some_new_event(5.0, 2.0, 12.0));
            assert_eq!(iter.next(), some_new_event(8.0, 1.0, 7.0));
            assert_eq!(iter.next(), some_new_event(11.0, 2.0, 6.0));
            assert_eq!(iter.next(), some_new_event(13.0, 5.0, 8.0));
            assert_eq!(iter.next(), some_new_event(15.0, 8.0, 11.0));
            assert_eq!(iter.next(), None);
        }
    }

    mod end_duration {
        use super::*;
        const DATA: [Intensity; 17] = [4, 3, 2, 5, 8, 12, 2, 1, 5, 7, 2, 6, 5, 8, 8, 11, 0];

        #[test]
        fn test_duration_3() {
            let parameters = DifferentialThresholdParameters {
                begin_threshold: 2.5,
                end_threshold: 0.0,
                end_duration: 3.0,
                ..Default::default()
            };
            let detector = DifferentialThresholdDetector::new(&parameters, Default::default());
            let mut iter = pipeline(&DATA, detector);

            assert_eq!(iter.next(), some_new_event(5.0, 2.0, 12.0));
            assert_eq!(iter.next(), None);
        }

        #[test]
        fn test_duration_2() {
            let parameters = DifferentialThresholdParameters {
                begin_threshold: 2.5,
                end_threshold: 0.0,
                end_duration: 2.0,
                ..Default::default()
            };
            let detector = DifferentialThresholdDetector::new(&parameters, Default::default());
            let mut iter = pipeline(&DATA, detector);

            assert_eq!(iter.next(), some_new_event(5.0, 2.0, 12.0));
            assert_eq!(iter.next(), some_new_event(11.0, 2.0, 6.0));
            assert_eq!(iter.next(), None);
        }

        #[test]
        fn test_duration_1() {
            let parameters = DifferentialThresholdParameters {
                begin_threshold: 2.5,
                end_threshold: 0.0,
                end_duration: 1.0,
                ..Default::default()
            };
            let detector = DifferentialThresholdDetector::new(&parameters, Default::default());

            let mut iter = pipeline(&DATA, detector);
            assert_eq!(iter.next(), some_new_event(5.0, 2.0, 12.0));
            assert_eq!(iter.next(), some_new_event(8.0, 1.0, 7.0));
            assert_eq!(iter.next(), some_new_event(13.0, 5.0, 8.0));
            assert_eq!(iter.next(), None);
        }

        #[test]
        fn test_duration_0() {
            let parameters = DifferentialThresholdParameters {
                begin_threshold: 2.5,
                end_duration: 0.0,
                ..Default::default()
            };
            let detector = DifferentialThresholdDetector::new(&parameters, Default::default());

            let mut iter = pipeline(&DATA, detector);
            assert_eq!(iter.next(), some_new_event(5.0, 2.0, 12.0));
            assert_eq!(iter.next(), some_new_event(8.0, 1.0, 7.0));
            assert_eq!(iter.next(), some_new_event(11.0, 2.0, 6.0));
            assert_eq!(iter.next(), some_new_event(13.0, 5.0, 8.0));
            assert_eq!(iter.next(), some_new_event(15.0, 8.0, 11.0));
            assert_eq!(iter.next(), None);
        }
    }

    mod cool_off {
        use super::*;
        const DATA: [Intensity; 15] = [4, 3, 2, 5, 2, 1, 5, 7, 2, 6, 5, 8, 8, 11, 0];
        // The positive derivatives occur at:               3, 6, 7, 9,  11, 13
        // The derivatives greater then 2.5 occur at :      3, 6,    9,  11, 13
        // The following non-positive derivatives occur at: 4, 8,    10, 12, 14
        // For a cool down of 3 samples, we expect detections at t = 3, 9.
        // For a cool down of 2 samples, we expect detections at t = 3, 9, 11.
        // For a cool down of 1 samples, we expect detections at t = 3, 6, 11.
        // For a cool down of 0 samples, we expect detections at t = 3, 6, 9, 11, 13.

        #[test]
        fn test_cool_off_3() {
            let parameters = DifferentialThresholdParameters {
                begin_threshold: 2.5,
                end_threshold: 0.0,
                cool_off: 3.0,
                ..Default::default()
            };
            let detector = DifferentialThresholdDetector::new(&parameters, Default::default());
            let mut iter = pipeline(&DATA, detector);

            assert_eq!(iter.next(), some_new_event(3.0, 2.0, 5.0));
            assert_eq!(iter.next(), some_new_event(9.0, 2.0, 6.0));
            assert_eq!(iter.next(), None);
        }

        #[test]
        fn test_cool_off_2() {
            let parameters = DifferentialThresholdParameters {
                begin_threshold: 2.5,
                end_threshold: 0.0,
                cool_off: 2.0,
                ..Default::default()
            };
            let detector = DifferentialThresholdDetector::new(&parameters, Default::default());
            let mut iter = pipeline(&DATA, detector);

            assert_eq!(iter.next(), some_new_event(3.0, 2.0, 5.0));
            assert_eq!(iter.next(), some_new_event(9.0, 2.0, 6.0));
            assert_eq!(iter.next(), some_new_event(13.0, 8.0, 11.0));
            assert_eq!(iter.next(), None);
        }

        #[test]
        fn test_cool_off_1() {
            let parameters = DifferentialThresholdParameters {
                begin_threshold: 2.5,
                end_threshold: 0.0,
                cool_off: 1.0,
                ..Default::default()
            };
            let detector = DifferentialThresholdDetector::new(&parameters, Default::default());

            let mut iter = pipeline(&DATA, detector);
            assert_eq!(iter.next(), some_new_event(3.0, 2.0, 5.0));
            assert_eq!(iter.next(), some_new_event(6.0, 1.0, 7.0));
            assert_eq!(iter.next(), some_new_event(11.0, 5.0, 8.0));
            assert_eq!(iter.next(), None);
        }

        #[test]
        fn test_cool_off_0() {
            let parameters = DifferentialThresholdParameters {
                begin_threshold: 2.5,
                end_threshold: 0.0,
                ..Default::default()
            };
            let detector = DifferentialThresholdDetector::new(&parameters, Default::default());

            let mut iter = pipeline(&DATA, detector);
            assert_eq!(iter.next(), some_new_event(3.0, 2.0, 5.0));
            assert_eq!(iter.next(), some_new_event(6.0, 1.0, 7.0));
            assert_eq!(iter.next(), some_new_event(9.0, 2.0, 6.0));
            assert_eq!(iter.next(), some_new_event(11.0, 5.0, 8.0));
            assert_eq!(iter.next(), some_new_event(13.0, 8.0, 11.0));
            assert_eq!(iter.next(), None);
        }
    }

    mod b2b {
        use super::*;

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
            (normalising_factor * (rising_exp * rising_erfc + falling_exp * falling_erfc))
                as Intensity
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
                &DifferentialThresholdParameters {
                    begin_threshold: 3.0,
                    end_threshold: 0.0,
                    ..Default::default()
                },
                Default::default(),
            );
            let mut iter = pipeline(&data, detector);
            assert_eq!(iter.next(), some_new_event(17.0, 3.0, 112.0));
            assert_eq!(iter.next(), some_new_event(50.0, 4.0, 113.0));
            assert_eq!(iter.next(), some_new_event(77.0, 3.0, 111.0));
            assert_eq!(iter.next(), None);
        }
    }
}
