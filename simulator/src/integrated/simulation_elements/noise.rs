use std::collections::VecDeque;

use super::{Interval, NumExpression, utils::JsonValueError};
use chrono::Utc;
use digital_muon_common::Time;
use rand::SeedableRng;
use rand_distr::{Distribution, Normal};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct NoiseSource {
    bounds: Interval<NumExpression<Time>>,
    attributes: NoiseAttributes,
    /// Length of the moving average window to apply to the noise.
    /// If no smoothing is required, set this to
    /// ```json
    /// "smoothing-window-length": { "const": 1 }
    /// ```
    smoothing_window_length: NumExpression<usize>,
}

impl NoiseSource {
    pub(crate) fn sample(&self, time: Time, frame_index: usize) -> Result<f64, JsonValueError> {
        if self.bounds.is_in(time, frame_index)? {
            match &self.attributes {
                NoiseAttributes::Uniform(Interval { min, max }) => {
                    let val = (max.value(frame_index)? - min.value(frame_index)?)
                        * rand::random::<f64>()
                        + min.value(frame_index)?;
                    Ok(val)
                }
                NoiseAttributes::Gaussian { mean, sd } => {
                    let val = Normal::new(mean.value(frame_index)?, sd.value(frame_index)?)?
                        .sample(&mut rand::rngs::StdRng::seed_from_u64(
                            Utc::now().timestamp_subsec_nanos() as u64,
                        ));
                    Ok(val)
                }
            }
        } else {
            Ok(f64::default())
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "noise-type")]
pub(crate) enum NoiseAttributes {
    Uniform(Interval<NumExpression<f64>>),
    Gaussian {
        mean: NumExpression<f64>,
        sd: NumExpression<f64>,
    },
}

pub(crate) struct Noise<'a> {
    source: &'a NoiseSource,
    prev: VecDeque<f64>,
}

impl<'a> Noise<'a> {
    pub(crate) fn new(source: &'a NoiseSource) -> Self {
        Self {
            source,
            prev: Default::default(),
        }
    }

    pub(crate) fn noisify(
        &mut self,
        value: f64,
        time: Time,
        frame_index: usize,
    ) -> Result<f64, JsonValueError> {
        let window_len = self.source.smoothing_window_length.value(frame_index)?;
        if self.prev.len() == window_len {
            self.prev.pop_front();
        }
        self.prev.push_back(self.source.sample(time, frame_index)?);
        Ok(value + self.prev.iter().sum::<f64>() / self.prev.len() as f64)
    }
}
