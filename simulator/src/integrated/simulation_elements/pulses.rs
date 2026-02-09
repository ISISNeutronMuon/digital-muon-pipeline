use core::f64;

use super::{FloatRandomDistribution, utils::JsonValueError};
use digital_muon_common::{Intensity, Time};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case", tag = "pulse-type")]
pub(crate) enum PulseTemplate {
    Flat {
        start: FloatRandomDistribution<f64>,
        width: FloatRandomDistribution<f64>,
        height: FloatRandomDistribution<f64>,
    },
    Triangular {
        start: FloatRandomDistribution<f64>,
        peak_time: FloatRandomDistribution<f64>,
        width: FloatRandomDistribution<f64>,
        height: FloatRandomDistribution<f64>,
    },
    Gaussian {
        height: FloatRandomDistribution<f64>,
        peak_time: FloatRandomDistribution<f64>,
        sd: FloatRandomDistribution<f64>,
    },
    BackToBackExp {
        peak_height: FloatRandomDistribution<f64>,
        peak_time: FloatRandomDistribution<f64>,
        spread: FloatRandomDistribution<f64>,
        decay: FloatRandomDistribution<f64>,
        rise: FloatRandomDistribution<f64>,
    },
}

#[derive(Debug)]
pub(crate) enum PulseEvent {
    Flat {
        start: f64,
        stop: f64,
        amplitude: f64,
    },
    Triangular {
        start: f64,
        peak_time: f64,
        stop: f64,
        amplitude: f64,
    },
    Gaussian {
        start: f64,
        stop: f64,
        mean: f64,
        sd: f64,
        peak_amplitude: f64,
    },
    BackToBackExp {
        start: f64,
        stop: f64,
        peak_time: f64,
        falling: f64,
        rising: f64,
        normalising_factor: f64,
        rising_spread: f64,
        falling_spread: f64,
        frac_1_sqrt_2_spread: f64,
    },
}

impl PulseEvent {
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
                let distance_to_value_of_one = 2.0*sd*peak_amplitude.ln().sqrt();
                Ok(Self::Gaussian {
                    start: mean - distance_to_value_of_one,
                    stop: mean + distance_to_value_of_one,
                    mean,
                    sd,
                    peak_amplitude,
                })
            },
            PulseTemplate::BackToBackExp {
                peak_height,
                peak_time,
                spread,
                decay,
                rise,
            } => {
                let rising = rise.sample(frame)?;
                let falling = decay.sample(frame)?;
                let peak_height = peak_height.sample(frame)?;
                let spread = spread.sample(frame)?;
                let peak_time = peak_time.sample(frame)?;

                let rising_spread =  rising * spread.powi(2);
                let falling_spread = falling * spread.powi(2);
                let frac_1_sqrt_2_spread = std::f64::consts::FRAC_1_SQRT_2 / spread;

                let normalising_factor = {
                    let rising_exp = f64::exp(rising * (0.5 * rising_spread));
                    let rising_erfc =
                        libm::erfc(rising_spread * frac_1_sqrt_2_spread);
                    let falling_exp = f64::exp(falling * (0.5 * falling_spread));
                    let falling_erfc =
                        libm::erfc(falling_spread * frac_1_sqrt_2_spread);

                    peak_height/(rising_exp * rising_erfc + falling_exp * falling_erfc)
                };

                let start = peak_time - 0.5 * rising_spread - normalising_factor.ln()/rising;
                let stop = peak_time + 0.5 * falling_spread + normalising_factor.ln()/falling;

                Ok(Self::BackToBackExp {
                    start,
                    stop,
                    peak_time,
                    falling,
                    rising,
                    normalising_factor,
                    rising_spread,
                    falling_spread,
                    frac_1_sqrt_2_spread: std::f64::consts::FRAC_1_SQRT_2 / spread,
                })
            }
        }
    }

    pub(crate) fn get_start(&self) -> Time {
        (match self {
            Self::Flat { start, .. } => *start,
            Self::Triangular { start, .. } => *start,
            Self::Gaussian { start, .. } => *start,
            Self::BackToBackExp { start, .. } => *start,
        }) as Time
    }

    pub(crate) fn get_end(&self) -> Time {
        (match self {
            Self::Flat { stop, .. } => *stop,
            Self::Triangular { stop, .. } => *stop,
            Self::Gaussian { stop, .. } => *stop,
            Self::BackToBackExp { stop, .. } => *stop,
        }) as Time
    }

    pub(crate) fn time(&self) -> Time {
        (match self {
            Self::Flat { start, .. } => *start,
            Self::Triangular { peak_time, .. } => *peak_time,
            Self::Gaussian { mean, .. } => *mean,
            Self::BackToBackExp {
                start, peak_time, ..
            } => *start + *peak_time / 2.0,
        }) as Time
    }

    pub(crate) fn intensity(&self) -> Intensity {
        (match self {
            Self::Flat { amplitude, .. } => *amplitude,
            Self::Triangular { amplitude, .. } => *amplitude,
            Self::Gaussian { peak_amplitude, .. } => *peak_amplitude,
            Self::BackToBackExp { falling, rising, normalising_factor, rising_spread, falling_spread, frac_1_sqrt_2_spread, .. } => {
                    let rising_exp = f64::exp(rising * (0.5 * rising_spread));
                    let rising_erfc =
                        libm::erfc(rising_spread * frac_1_sqrt_2_spread);
                    let falling_exp = f64::exp(falling * (0.5 * falling_spread));
                    let falling_erfc =
                        libm::erfc(falling_spread * frac_1_sqrt_2_spread);

                    normalising_factor * (rising_exp * rising_erfc + falling_exp * falling_erfc)
            },
        }) as Intensity
    }

    pub(crate) fn get_value_at(&self, time: f64) -> f64 {
        match *self {
            Self::Flat {
                start,
                stop,
                amplitude,
            } => {
                if start <= time && time < stop {
                    amplitude
                } else {
                    f64::default()
                }
            }
            Self::Triangular {
                start,
                peak_time,
                stop,
                amplitude,
            } => {
                if start <= time && time < peak_time {
                    amplitude * (time - start) / (peak_time - start)
                } else if peak_time <= time && time < stop {
                    amplitude * (stop - time) / (stop - peak_time)
                } else {
                    f64::default()
                }
            }
            Self::Gaussian {
                mean,
                sd,
                peak_amplitude,
                start,
                stop
            } => {
                if start > time || time > stop {
                    f64::default()
                } else {
                    peak_amplitude * f64::exp(-f64::powi(0.5 * (time - mean) / sd, 2))
                }
            }
            Self::BackToBackExp {
                start,
                stop,
                peak_time,
                falling,
                rising,
                normalising_factor,
                rising_spread,
                falling_spread,
                frac_1_sqrt_2_spread,
            } => {
                if start < time || time < stop {
                    let time_shift = time - peak_time;

                    let rising_exp = f64::exp(rising * (0.5 * rising_spread + time_shift));
                    let rising_erfc =
                        libm::erfc((rising_spread + time_shift) * frac_1_sqrt_2_spread);
                    let falling_exp = f64::exp(falling * (0.5 * falling_spread - time_shift));
                    let falling_erfc =
                        libm::erfc((falling_spread - time_shift) * frac_1_sqrt_2_spread);
                    
                    normalising_factor * (rising_exp * rising_erfc + falling_exp * falling_erfc)
                } else {
                    Default::default()
                }
            }
        }
    }
}
