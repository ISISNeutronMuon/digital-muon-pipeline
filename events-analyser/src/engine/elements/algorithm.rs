use crate::engine::{
    Array, FlatWaveform, FlattenableWithIndex, HasName,
    values::{Value, ValueError},
};
use digital_muon_common::{Intensity, Time};
use serde::Deserialize;
use std::{fmt::Debug, ops::Deref};

/// Encapsulates all properties which defines a `trace-to-events` algorithm,
/// used in event detection. The evaluator presumes these settings were used to
/// detect the events it is evaluating in a particular bucket.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Algorithm {
    // Name of the algorithm properties.
    pub(crate) name: String,
    #[serde(flatten)]
    // Properties defining the algorithm.
    pub(crate) properties: AlgorithmProperties,
}

impl HasName for Algorithm {
    fn get_name(&self) -> &str {
        &self.name
    }
}

impl Deref for Algorithm {
    type Target = AlgorithmProperties;

    fn deref(&self) -> &Self::Target {
        &self.properties
    }
}

/// Encapsulates all properties which defines a `trace-to-events` algorithm,
/// used in event detection. The evaluator presumes these settings were used to
/// detect the events it is evaluating in a particular bucket.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AlgorithmProperties {
    /// Specifies the fixed threshold discriminator algorithm.
    FixedThreshold {
        /// The threshold property used in the event detection.
        threshold: Value<f64>,
        /// The duration property used in the event detection.
        duration: Value<Time>,
        /// The cool_down property used in the event detection.
        cool_down: Value<Time>,
    },
}

impl FlattenableWithIndex for AlgorithmProperties {
    type Flat = FlatAlgorithm;
    type Library = [Array];
    type Error = ValueError;

    fn flatten(&self, arrays: &Self::Library, index: usize) -> Result<FlatAlgorithm, Self::Error> {
        match self {
            Self::FixedThreshold {
                threshold,
                duration,
                cool_down,
            } => {
                let _threshold = threshold.flatten(arrays, index)?;
                let _duration = duration.flatten(arrays, index)?;
                let _cool_down = cool_down.flatten(arrays, index)?;
                Ok(FlatAlgorithm::FixedThreshold {
                    _threshold,
                    _duration,
                    _cool_down,
                })
            }
        }
    }
}

/// The version of `AlgorithmProperties` with all dependencies resolved.
#[derive(Debug, Clone)]
pub(crate) enum FlatAlgorithm {
    /// Specifies the fixed threshold discriminator algorithm.
    FixedThreshold {
        /// The threshold property used in the event detection.
        _threshold: f64,
        /// The duration property used in the event detection.
        _duration: Time,
        /// The cool_down property used in the event detection.
        _cool_down: Time,
    },
}

impl FlatAlgorithm {
    /// Estimates whether the given (time,intensity) pair could been detected by this algorithm
    /// applied to a given [FlatWaveform] with given peak time and peak intensity.
    /// # Parameters
    /// - waveform: Waveform to model the pulse by.
    /// - detected: Waveform to model the pulse by.
    pub(crate) fn _is_true_positive(
        &self,
        waveform: &FlatWaveform,
        detected: (Time, Intensity),
        pulse_peak: (Time, Intensity),
    ) -> bool {
        match self {
            &FlatAlgorithm::FixedThreshold {
                _threshold,
                _duration,
                _cool_down,
            } => {
                let height = _threshold / pulse_peak.1 as f64;
                let width = waveform._effective_radius_at_proportion_of_peak(height);
                (detected.0 as f64 - pulse_peak.0 as f64).abs()
                    < (_duration + _cool_down) as f64 + width
            }
        }
    }
}
