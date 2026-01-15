//! This detector registeres events when the trace differential exceeds a certain value (indicating onset),
//! reaches zero (indicating the signal peak), and falls to a given value (indicating the pulse end).
use super::{Assembler, Detector, EventData, EventPoint, Pulse, Real, RealArray, TimeValue};

/// Indicates the type of event.
#[derive(Default, Debug, Clone, PartialEq)]
pub(crate) enum Class {
    /// Indicates a pulse has started (i.e. is rising).
    #[default]
    Onset,
    /// Indicating a pulse has peaked.
    Peak,
    /// Indicating a pulse has ended.
    End,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub(crate) struct Data {
    /// The type of event.
    class: Class,
    value: Real,
    superlative: Option<TimeValue<RealArray<2>>>,
}

impl Data {
    pub(crate) fn get_class(&self) -> Class {
        self.class.clone()
    }

    pub(crate) fn get_value(&self) -> Real {
        self.value
    }

    pub(crate) fn get_superlative(&self) -> Option<TimeValue<RealArray<2>>> {
        self.superlative.clone()
    }
}

impl EventData for Data {}

type BasicMuonEvent = (Real, Data);

type SuperlativeValue = TimeValue<Real>;

type SuperlativeDiff = TimeValue<RealArray<2>>;

#[derive(Clone, Debug, PartialEq)]
enum Mode {
    Rise,
    Fall,
}

/// The current state of the detector.
#[derive(Clone, Debug)]
struct State(Mode, SuperlativeValue, SuperlativeDiff);

impl State {
    fn from_mode(mode: Option<Mode>, time: Real, value: &RealArray<2>) -> Option<Self> {
        mode.map(|mode| {
            State(
                mode,
                SuperlativeValue {
                    time,
                    value: value[0],
                },
                SuperlativeDiff {
                    time,
                    value: *value,
                },
            )
        })
    }

    fn test_and_update_superlative(&mut self, time: Real, value: &RealArray<2>) {
        match self {
            State(Mode::Rise, peak, steepest_rise) => {
                //  Update Steepest Rise
                if value[1] >= steepest_rise.value[1] {
                    steepest_rise.time = time;
                    steepest_rise.value = *value;
                }
                //  Update Peak
                if value[0] >= peak.value {
                    peak.time = time;
                    peak.value = value[0];
                }
            }
            State(Mode::Fall, nadir, sharpest_fall) => {
                if value[1] <= sharpest_fall.value[1] {
                    sharpest_fall.time = time;
                    sharpest_fall.value = *value;
                }
                //  Update Nadir
                if value[0] <= nadir.value {
                    nadir.time = time;
                    nadir.value = value[0];
                }
            }
        }
    }

    fn generate_event(&self) -> BasicMuonEvent {
        let State(mode, extreme, extreme_diff) = self;
        (
            extreme.time,
            Data {
                class: match mode {
                    Mode::Rise => Class::Peak,
                    Mode::Fall => Class::End,
                },
                value: extreme.value,
                superlative: Some(extreme_diff.clone()),
            },
        )
    }
}

#[derive(Default, Clone)]
pub(crate) struct AdvancedMuonDetector {
    onset_threshold: Real,
    fall_threshold: Real,
    termination_threshold: Real,
    duration: Real,

    // If a change in signal behavior is detected then it is recorded in potential_mode.
    //If the change lasts the requisite duration then the mode is changed.
    state: Option<State>,
    time_crossed: Option<Real>,
}

impl AdvancedMuonDetector {
    pub(crate) fn new(onset: Real, fall: Real, termination: Real, duration: Real) -> Self {
        Self {
            onset_threshold: onset,
            fall_threshold: fall,
            termination_threshold: termination,
            duration,
            ..Default::default()
        }
    }

    fn test_threshold(&self, value: &RealArray<2>) -> bool {
        match &self.state {
            Some(State(Mode::Rise, _, _)) => value[1] <= self.fall_threshold,
            Some(State(Mode::Fall, _, _)) => value[1] >= self.termination_threshold,
            None => value[1] >= self.onset_threshold,
        }
    }

    fn test_threshold_duration(&self, time: Real) -> bool {
        self.time_crossed
            .map(|time_crossed| time - time_crossed >= self.duration)
            .unwrap_or(false)
    }

    fn test_and_update_threshold(&mut self, time: Real, value: &RealArray<2>) {
        if self.time_crossed.is_some() {
            if !self.test_threshold(value) {
                self.time_crossed = None;
            }
        } else if self.test_threshold(value) {
            self.time_crossed = Some(time);
        }
    }
}

impl Detector for AdvancedMuonDetector {
    type TracePointType = (Real, RealArray<2>);
    type EventPointType = (Real, Data);

    fn signal(&mut self, time: Real, value: RealArray<2>) -> Option<BasicMuonEvent> {
        self.test_and_update_threshold(time, &value);
        if let Some(state) = &mut self.state {
            state.test_and_update_superlative(time, &value);
        }
        match &self.state {
            Some(state) => {
                if self.test_threshold_duration(time) {
                    let event = state.generate_event();
                    let State(mode, _, _) = &state;
                    self.state = State::from_mode(
                        match mode {
                            Mode::Rise => Some(Mode::Fall),
                            Mode::Fall => None,
                        },
                        time,
                        &value,
                    );
                    Some(event)
                } else {
                    None
                }
            }
            None => {
                if self.test_threshold_duration(time) {
                    let event = (
                        time,
                        Data {
                            class: Class::Onset,
                            value: value[0],
                            ..Default::default()
                        },
                    );
                    self.state = State::from_mode(Some(Mode::Rise), time, &value);
                    Some(event)
                } else {
                    None
                }
            }
        }
    }

    fn finish(&mut self) -> Option<Self::EventPointType> {
        None
    }
}

#[derive(Default, Clone, Debug)]
enum AssemblerMode {
    #[default]
    Waiting,
    Rising {
        start: TimeValue<Real>,
    },
    Falling {
        start: TimeValue<Real>,
        steepest_rise: Option<TimeValue<RealArray<2>>>,
        peak: TimeValue<Real>,
    },
}

#[derive(Default, Clone)]
pub(crate) struct AdvancedMuonAssembler {
    mode: AssemblerMode,
}

impl Assembler for AdvancedMuonAssembler {
    type DetectorType = AdvancedMuonDetector;

    fn assemble_pulses(&mut self, source: (Real, Data)) -> Option<Pulse> {
        match self.mode.clone() {
            AssemblerMode::Waiting => match source.get_data().get_class() {
                Class::Onset => {
                    let start = TimeValue {
                        time: source.get_time(),
                        value: source.get_data().get_value(),
                    };
                    self.mode = AssemblerMode::Rising { start };
                    None
                }
                _ => None,
            },
            AssemblerMode::Rising { start } => match source.get_data().get_class() {
                Class::Peak => {
                    let peak = TimeValue::<Real> {
                        time: source.get_time(),
                        value: source.get_data().get_value(),
                    };
                    self.mode = AssemblerMode::Falling {
                        start,
                        steepest_rise: source.get_data().get_superlative(),
                        peak,
                    };
                    None
                }
                _ => None,
            },
            AssemblerMode::Falling {
                start,
                steepest_rise,
                mut peak,
            } => match source.get_data().get_class() {
                Class::End => {
                    self.mode = AssemblerMode::Waiting;
                    Some(TimeValue {
                        time: source.get_time(),
                        value: source.get_data().get_value(),
                    })
                }
                _ => None,
            }
            .map(|end| {
                let mut steepest_rise = steepest_rise.unwrap_or_default();
                let mut sharpest_fall = source.get_data().get_superlative().unwrap_or_default();

                let gradient = (peak.time - start.time) / (end.time - start.time);
                peak.value -= (peak.value - start.value) * gradient;
                steepest_rise.value[0] -= (steepest_rise.value[0] - start.value) * gradient;
                sharpest_fall.value[0] -= (sharpest_fall.value[0] - start.value) * gradient;

                Pulse {
                    start: start.into(),
                    peak: peak.into(),
                    end: end.into(),
                    steepest_rise: steepest_rise.into(),
                    sharpest_fall: sharpest_fall.into(),
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse_detection::{
        EventFilter, WindowFilter, datatype::TraceArray, window::FiniteDifferences,
    };

    #[test]
    fn test_threshold() {
        let data = [4, 3, 2, 5, 6, 1, 5, 7, 2, 4];
        let detector = AdvancedMuonDetector::new(1.0, 1.0, 1.0, 0.0);
        let mut iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(FiniteDifferences::<2>::new())
            .events(detector);
        assert_eq!(
            iter.next(),
            Some((
                3.0,
                Data {
                    class: Class::Onset,
                    value: 5.0,
                    superlative: None
                }
            ))
        );
        assert_eq!(
            iter.next(),
            Some((
                4.0,
                Data {
                    class: Class::Peak,
                    value: 6.0,
                    superlative: Some(TimeValue {
                        time: 3.0,
                        value: TraceArray([5.0, 3.0])
                    })
                }
            ))
        );
        assert_eq!(
            iter.next(),
            Some((
                5.0,
                Data {
                    class: Class::End,
                    value: 1.0,
                    superlative: Some(TimeValue {
                        time: 5.0,
                        value: TraceArray([1.0, -5.0])
                    })
                }
            ))
        );
        assert_eq!(
            iter.next(),
            Some((
                7.0,
                Data {
                    class: Class::Onset,
                    value: 7.0,
                    superlative: None
                }
            ))
        );
        assert_eq!(
            iter.next(),
            Some((
                7.0,
                Data {
                    class: Class::Peak,
                    value: 7.0,
                    superlative: Some(TimeValue {
                        time: 7.0,
                        value: TraceArray([7.0, 2.0])
                    })
                }
            ))
        );
        assert_eq!(
            iter.next(),
            Some((
                8.0,
                Data {
                    class: Class::End,
                    value: 2.0,
                    superlative: Some(TimeValue {
                        time: 8.0,
                        value: TraceArray([2.0, -5.0])
                    })
                }
            ))
        );
        assert_eq!(iter.next(), None);
    }
}
