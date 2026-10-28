use std::collections::VecDeque;

use crate::integrated::simulation_elements::FloatRandomDistribution;

use super::{Interval, NumExpression, utils::JsonValueError};
use chrono::Utc;
use digital_muon_common::Time;
use rand::SeedableRng;
use rand_distr::{Distribution, Normal};
use serde::Deserialize;

/// Source of noise.
///
/// Should only be created via JSON deserialisation.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct NoiseSource {
    /// Time bins between which the noise is nonzero.
    bounds: Interval<NumExpression<Time>>,
    /// Defines noise type.
    attributes: NoiseAttributes,
    /// Length of the moving average window to apply to the noise.
    ///
    /// Note that [NoiseSource] does not apply this window, this field is used by [Noise].
    /// If no smoothing is required, set this to
    /// ```json
    /// "smoothing-window-length": { "const": 1 }
    /// ```
    smoothing_window_length: NumExpression<usize>,
}

impl NoiseSource {
    /// Generates a stateless noise value.
    ///
    /// # Parameters
    /// - time: current time bin.
    /// - frame_index: current frame, for quantities depending on [Transformation].
    pub(crate) fn sample(&self, time: Time, frame_index: usize) -> Result<f64, JsonValueError> {
        if self.bounds.is_in(time, frame_index)? {
            match &self.attributes {
                NoiseAttributes::Bernoulli { probability, value } => {
                    if rand::random_bool(probability.value(frame_index)?) {
                        value.sample(frame_index)
                    } else {
                        Ok(0.0)
                    }
                }
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

/// Defines the type of randomness used in [NoiseSource].
///
/// Should only be created via JSON deserialisation.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "noise-type")]
pub(crate) enum NoiseAttributes {
    /// Bernoulli noise, that is random blips of `value`.
    Bernoulli {
        /// How likely is a given timebin to be nonzero.
        probability: NumExpression<f64>,
        /// Value of the blip.
        value: FloatRandomDistribution<f64>,
    },
    /// Uniform noise, nonzero within the interval.
    Uniform(Interval<NumExpression<f64>>),
    /// Gaussian noise.
    Gaussian {
        mean: NumExpression<f64>,
        sd: NumExpression<f64>,
    },
}

/// Stateful noise, this is created from a [NoiseSource].
pub(crate) struct Noise<'a> {
    /// Source from which the noise is created.
    source: &'a NoiseSource,
    /// Window of previous noise values, has length [NoiseSource::smoothing_window_length].
    prev: VecDeque<f64>,
}

impl<'a> Noise<'a> {
    /// Create new noise from source.
    ///
    /// # Parameters
    /// - source: noise source.
    pub(crate) fn new(source: &'a NoiseSource) -> Self {
        Self {
            source,
            prev: Default::default(),
        }
    }

    /// Adds noise to an underlying value.
    ///
    /// # Parameters
    /// - value: underlying value to add noise to.
    /// - time: current time bin.
    /// - frame_index: current frame, for quantities depending on [Transformation].
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
