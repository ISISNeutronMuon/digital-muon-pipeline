use core::f64;

use super::{FloatRandomDistribution, utils::JsonValueError};
use digital_muon_common::{Intensity, Time};
use serde::Deserialize;

/// Defines a template for a pulse that can be realised into [PulseEvent].
///
/// Should only be created via JSON deserialisation.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case", tag = "pulse-type")]
pub(crate) enum PulseTemplate {
    /// Flat waveform.
    Flat {
        start: FloatRandomDistribution<f64>,
        width: FloatRandomDistribution<f64>,
        height: FloatRandomDistribution<f64>,
    },
    /// Triangular waveform.
    Triangular {
        start: FloatRandomDistribution<f64>,
        peak_time: FloatRandomDistribution<f64>,
        width: FloatRandomDistribution<f64>,
        height: FloatRandomDistribution<f64>,
    },
    /// Gaussian waveform.
    Gaussian {
        height: FloatRandomDistribution<f64>,
        peak_time: FloatRandomDistribution<f64>,
        sd: FloatRandomDistribution<f64>,
    },
    /// Back-to-Back Exponential waveform.
    BackToBackExp {
        peak_height: FloatRandomDistribution<f64>,
        peak_time: FloatRandomDistribution<f64>,
        spread: FloatRandomDistribution<f64>,
        falling: FloatRandomDistribution<f64>,
        rising: FloatRandomDistribution<f64>,
    },
}

/// A concrete instance of a pulse, appearing in [EventList].
#[derive(Debug, Clone)]
pub(crate) enum PulseEvent {
    /// Flat waveform.
    Flat {
        /// First nonzero time-bin.
        start: f64,
        /// Last nonzero time-bin.
        stop: f64,
        /// Size of waveform.
        amplitude: f64,
    },
    /// Triangular waveform.
    Triangular {
        /// First nonzero time-bin.
        start: f64,
        /// Timebin at which the triangle peaks.
        peak_time: f64,
        /// Last nonzero time-bin.
        stop: f64,
        /// Maximum height of waveform.
        amplitude: f64,
    },
    /// Gaussian waveform.
    Gaussian {
        /// First nonzero time-bin.
        start: f64,
        /// Last nonzero time-bin.
        stop: f64,
        /// Mean of Gaussian.
        mean: f64,
        /// Standard Deviation of Gaussian.
        sd: f64,
        /// Maximum height of waveform.
        peak_amplitude: f64,
    },
    /// Back-to-Back Exponential waveform.
    BackToBackExp {
        /// First nonzero time-bin.
        start: f64,
        /// Last nonzero time-bin.
        stop: f64,
        /// Timebin at which peak occurs.
        peak_time: f64,
        /// Coefficient of falling exponential.
        falling: f64,
        /// Coefficient of rising exponential.
        rising: f64,
        ///
        normalising_factor: f64,
        ///
        rising_spread: f64,
        ///
        falling_spread: f64,
        ///
        frac_1_sqrt_2_spread: f64,
    },
}

impl PulseEvent {
    /// Create new pulse event from a given pulse template and frame number.
    ///
    /// # Parameters
    /// - template: underlying template to realise.
    /// - frame: current frame, for quantities depending on [Transformation].
    pub(crate) fn sample(template: &PulseTemplate, frame: usize) -> Result<Self, JsonValueError> {
        match template {
            PulseTemplate::Flat {
                start,
                width,
                height,
            } => {
                let start = start.sample(frame)?;
                Ok(Self::Flat {
                    start,
                    stop: start + width.sample(frame)?,
                    amplitude: height.sample(frame)?,
                })
            }
            PulseTemplate::Triangular {
                start,
                peak_time,
                width,
                height,
            } => {
                let start = start.sample(frame)?;
                let width = width.sample(frame)?;
                Ok(Self::Triangular {
                    start,
                    peak_time: start + peak_time.sample(frame)? * width,
                    stop: start + width,
                    amplitude: height.sample(frame)?,
                })
            }
            PulseTemplate::Gaussian {
                height,
                peak_time,
                sd,
            } => {
                let mean = peak_time.sample(frame)?;
                let sd = sd.sample(frame)?;
                let peak_amplitude = height.sample(frame)?;
                let distance_to_value_of_one = 2.0 * sd * peak_amplitude.ln().sqrt();
                Ok(Self::Gaussian {
                    start: mean - distance_to_value_of_one,
                    stop: mean + distance_to_value_of_one,
                    mean,
                    sd,
                    peak_amplitude,
                })
            }
            PulseTemplate::BackToBackExp {
                peak_height,
                peak_time,
                spread,
                falling,
                rising,
            } => {
                let rising = rising.sample(frame)?;
                let falling = falling.sample(frame)?;
                let peak_height = peak_height.sample(frame)?;
                let spread = spread.sample(frame)?;
                let peak_time = peak_time.sample(frame)?;

                let rising_spread = rising * spread.powi(2);
                let falling_spread = falling * spread.powi(2);
                let frac_1_sqrt_2_spread = std::f64::consts::FRAC_1_SQRT_2 / spread;

                let normalising_factor = {
                    let rising_erfc = libm::erfc(rising_spread * frac_1_sqrt_2_spread);
                    let rising_exp = if rising_erfc == 0.0 {
                        0.0
                    } else {
                        f64::exp(0.5 * rising * rising_spread)
                    };
                    let falling_erfc = libm::erfc(falling_spread * frac_1_sqrt_2_spread);
                    let falling_exp = if falling_erfc == 0.0 {
                        0.0
                    } else {
                        f64::exp(0.5 * falling * falling_spread)
                    };

                    peak_height / (rising_exp * rising_erfc + falling_exp * falling_erfc)
                };

                let start = peak_time - 0.5 * rising_spread - normalising_factor.ln() / rising;
                let stop = peak_time + 0.5 * falling_spread + normalising_factor.ln() / falling;

                Ok(Self::BackToBackExp {
                    start,
                    stop,
                    peak_time,
                    falling,
                    rising,
                    normalising_factor,
                    rising_spread,
                    falling_spread,
                    frac_1_sqrt_2_spread,
                })
            }
        }
    }

    /// Get start field.
    pub(crate) fn get_start(&self) -> Time {
        (match self {
            Self::Flat { start, .. } => *start,
            Self::Triangular { start, .. } => *start,
            Self::Gaussian { start, .. } => *start,
            Self::BackToBackExp { start, .. } => *start,
        }) as Time
    }

    /// Get stop field.
    pub(crate) fn get_end(&self) -> Time {
        (match self {
            Self::Flat { stop, .. } => *stop,
            Self::Triangular { stop, .. } => *stop,
            Self::Gaussian { stop, .. } => *stop,
            Self::BackToBackExp { stop, .. } => *stop,
        }) as Time
    }

    /// Get time bin that maximum intensity first occurs.
    pub(crate) fn time(&self) -> Time {
        (match self {
            Self::Flat { start, .. } => *start,
            Self::Triangular { peak_time, .. } => *peak_time,
            Self::Gaussian { mean, .. } => *mean,
            Self::BackToBackExp { peak_time, .. } => *peak_time,
        }) as Time
    }

    /// Get value of maximum intensity.
    pub(crate) fn intensity(&self) -> Intensity {
        (match self {
            Self::Flat { amplitude, .. } => *amplitude,
            Self::Triangular { amplitude, .. } => *amplitude,
            Self::Gaussian { peak_amplitude, .. } => *peak_amplitude,
            Self::BackToBackExp {
                falling,
                rising,
                normalising_factor,
                rising_spread,
                falling_spread,
                frac_1_sqrt_2_spread,
                ..
            } => {
                let rising_exp = f64::exp(rising * (0.5 * rising_spread));
                let rising_erfc = libm::erfc(rising_spread * frac_1_sqrt_2_spread);
                let falling_exp = f64::exp(falling * (0.5 * falling_spread));
                let falling_erfc = libm::erfc(falling_spread * frac_1_sqrt_2_spread);

                normalising_factor * (rising_exp * rising_erfc + falling_exp * falling_erfc)
            }
        }) as Intensity
    }

    /// Get value of pulse at a given time.
    ///
    /// # Parameters
    /// - time: time bin to get value at.
    pub(crate) fn get_value_at(&self, time: f64) -> f64 {
        if self.get_start() > time as Time || time as Time > self.get_end() {
            return Default::default();
        }

        match *self {
            Self::Flat { amplitude, .. } => amplitude,
            Self::Triangular {
                start,
                peak_time,
                stop,
                amplitude,
            } => {
                if time < peak_time {
                    amplitude * (time - start) / (peak_time - start)
                } else {
                    amplitude * (stop - time) / (stop - peak_time)
                }
            }
            Self::Gaussian {
                mean,
                sd,
                peak_amplitude,
                ..
            } => peak_amplitude * f64::exp(-f64::powi(0.5 * (time - mean) / sd, 2)),
            Self::BackToBackExp {
                peak_time,
                falling,
                rising,
                normalising_factor,
                rising_spread,
                falling_spread,
                frac_1_sqrt_2_spread,
                ..
            } => {
                let time_shift = time - peak_time; // -2.9539084192897

                let rising_erfc = libm::erfc((rising_spread + time_shift) * frac_1_sqrt_2_spread);
                //  Guard against rising_exp being NaN or Inf
                let rising_exp = if rising_erfc >= f64::EPSILON {
                    f64::exp(rising * (0.5 * rising_spread + time_shift))
                } else {
                    Default::default()
                };

                let falling_erfc = libm::erfc((falling_spread - time_shift) * frac_1_sqrt_2_spread);
                //  Guard against falling_exp being NaN or Inf
                let falling_exp = if falling_erfc >= f64::EPSILON {
                    f64::exp(falling * (0.5 * falling_spread - time_shift))
                } else {
                    Default::default()
                };

                normalising_factor * (rising_exp * rising_erfc + falling_exp * falling_erfc)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::integrated::simulation_elements::NumExpression;

    use super::*;

    const TEMPLATE: PulseTemplate = PulseTemplate::BackToBackExp {
        peak_height: FloatRandomDistribution::ConstantFloat {
            value: NumExpression::Const(2100.0),
        },
        peak_time: FloatRandomDistribution::ConstantFloat {
            value: NumExpression::Const(2200.0),
        },
        spread: FloatRandomDistribution::ConstantFloat {
            value: NumExpression::Const(3.0),
        },
        falling: FloatRandomDistribution::ConstantFloat {
            value: NumExpression::Const(2.5),
        },
        rising: FloatRandomDistribution::ConstantFloat {
            value: NumExpression::Const(1.5),
        },
    };

    #[test]
    fn back_to_back_exp_template() {
        let pulse = PulseEvent::sample(&TEMPLATE, 0);
        assert!(pulse.is_ok());
        let pulse = pulse.unwrap();
        assert_eq!(pulse.get_start(), 2187);
        assert_eq!(pulse.get_end(), 2214);
        assert_eq!(pulse.intensity(), 2100);
        assert_eq!(pulse.time(), 2200);
    }

    #[test]
    fn back_to_back_exp_values() {
        let pulse = PulseEvent::sample(&TEMPLATE, 0).unwrap();
        const VALUES: [Intensity; 27] = [
            0, 1, 5, 14, 35, 78, 159, 292, 487, 730, 988, 1793, 2044, 2100, 1942, 1616, 1211, 816,
            495, 270, 132, 58, 23, 8, 2, 0, 0,
        ];
        for (t, &v) in VALUES.iter().enumerate() {
            assert_eq!(
                pulse.get_value_at((pulse.get_start() + t as Time) as f64) as Intensity,
                v
            );
        }
    }

    #[test]
    fn back_to_back_exp_extreme_rising() {
        let template = PulseTemplate::BackToBackExp {
            peak_height: FloatRandomDistribution::ConstantFloat {
                value: NumExpression::Const(2100.0),
            },
            peak_time: FloatRandomDistribution::ConstantFloat {
                value: NumExpression::Const(2200.0),
            },
            spread: FloatRandomDistribution::ConstantFloat {
                value: NumExpression::Const(3.0),
            },
            falling: FloatRandomDistribution::ConstantFloat {
                value: NumExpression::Const(12.5),
            },
            rising: FloatRandomDistribution::ConstantFloat {
                value: NumExpression::Const(1.5),
            },
        };
        let event = PulseEvent::sample(&template, 0).unwrap();
        const VALUES1: [Intensity; 27] = [
            0, 2, 7, 20, 50, 112, 228, 420, 699, 1049, 1419, 1728, 1893, 1866, 1652, 1314, 938,
            601, 346, 178, 82, 34, 12, 4, 1, 0, 0,
        ];
        for (t, &v) in VALUES1.iter().enumerate() {
            assert_eq!(
                event.get_value_at(event.get_start() as f64 + t as f64) as Intensity,
                v
            );
        }
    }

    #[test]
    fn back_to_back_exp_extreme_falling() {
        let template = PulseTemplate::BackToBackExp {
            peak_height: FloatRandomDistribution::ConstantFloat {
                value: NumExpression::Const(2100.0),
            },
            peak_time: FloatRandomDistribution::ConstantFloat {
                value: NumExpression::Const(2200.0),
            },
            spread: FloatRandomDistribution::ConstantFloat {
                value: NumExpression::Const(4.0),
            },
            falling: FloatRandomDistribution::ConstantFloat {
                value: NumExpression::Const(12.5),
            },
            rising: FloatRandomDistribution::ConstantFloat {
                value: NumExpression::Const(1.5),
            },
        };
        let event = PulseEvent::sample(&template, 0).unwrap();
        const VALUES2: [Intensity; 27] = [
            0, 0, 0, 1, 4, 10, 21, 43, 83, 151, 258, 414, 626, 892, 1197, 1512, 1798, 2012, 2119,
            2100, 1957, 1716, 1416, 1098, 801, 550, 355,
        ];
        for (t, &v) in VALUES2.iter().enumerate() {
            assert_eq!(
                event.get_value_at(event.get_start() as f64 + t as f64) as Intensity,
                v
            );
        }
    }
}
