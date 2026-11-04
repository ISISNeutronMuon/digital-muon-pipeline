//! Defines the parameters used by the various detectors defined in this component.
use crate::pulse_detection::Real;
use digital_muon_common::Intensity;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct DetectorSettings {
    /// The type of detector to use.
    pub(crate) mode: Mode,
    /// The polarity of the trace signal.
    pub(crate) polarity: Polarity,
    /// The baseline of the trace signal.
    #[serde(default)]
    pub(crate) baseline: Intensity,
}

/// Defines the polarity of the signal, i.e. whether events cause positive or negative signals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Polarity {
    /// Detection events register as positive signals.
    Positive,
    /// Detection events register as negative signals.
    Negative,
}

/// Encapsulates the parameters specific to the Fixed Threshold Discriminator detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FixedThresholdDiscriminatorParameters {
    /// If the detector is armed, an event is registered when the trace passes this value for the given duration.
    pub(crate) threshold: Real,

    /// The duration, in samples, that the trace must exceed the threshold for.
    pub(crate) duration: usize,

    /// After an event is registered, the detector disarms for this many samples.
    pub(crate) cool_off: usize,
}

/// Determines how the peak height is calculated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PeakHeightMode {
    /// Take the maximum trace value between begin trigger time and end trigger time.
    #[serde(alias = "max_value", alias = "MaxValue")]
    MaxValue,
    /// Take the trace value at the end trigger time.
    #[serde(alias = "value_at_end_trigger", alias = "ValueAtEndTrigger")]
    ValueAtEndTrigger,
}

/// Determines the peak height baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PeakHeightBasis {
    /// The peak height is relative to the trace's baseline.
    TraceBaseline,
    /// The peak height is relative to the pulse's baseline, i.e. the trace value at the time the pulse was first detected.
    PulseBaseline,
}

/// Encapsulates the parameters specific to the Differential Threshold Discriminator detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct DifferentialThresholdDiscriminatorParameters {
    /// If the detector is armed, an event is registered when the trace derivative passes this value for the given duration.
    pub(crate) begin_threshold: Real,

    /// The duration, in samples, that the trace derivative must exceed the begin threshold for a detection to begin.
    pub(crate) begin_duration: usize,

    /// If a detection is in progress, an event is concluded when the trace derivative passes below this value for the given duration.
    pub(crate) end_threshold: Real,

    /// The duration, in samples, that the trace derivative must drop below the end threshold for a detection to end.
    pub(crate) end_duration: usize,

    /// After an event is registered, the detector disarms for this many samples.
    pub(crate) cool_off: usize,

    /// Determines how the peak height is computed.
    pub(crate) peak_height_mode: PeakHeightMode,

    /// Determines how the peak height is computed.
    pub(crate) peak_height_basis: PeakHeightBasis,
}

/// Encapsulates the parameters specific to the Smoothing detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SmoothingDetectorParameters {
    /// Centile of x to use for noise estimation.
    pub(crate) noise_centile: Real,
    /// Sigma of the Gaussian kernel for smoothing.
    pub(crate) kernel_sigma: Real,
    /// Number of standard deviations above noise to use as threshold.
    pub(crate) nsig_noise: Real,
    /// Minimum size of region to consider a peak, if absent all regions are considered.
    pub(crate) min_size: Option<usize>,
    /// If set, then any region at or above this size will be converted to a list of local arg minima, rather than the global arg minimum.
    pub(crate) use_local_for_sizes_ge: Option<usize>,
}

/// Encapsulates the parameters specific to the Multiscaling detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct MultiscalingDetectorParameters {
    /// Coefficients of the smoothing kernel to apply on downsampling.
    pub(crate) downsampling_smoothing: Vec<Real>,
    /// Support of the `downsampling_smoothing` kernel used to compute the `upsampling_smoothing` kernel.
    pub(crate) smoothing_support: Vec<i32>,
    /// Amount of padding to use when calculating the `upsampling_smoothing` kernel.
    pub(crate) fft_padding: usize,
    /// Size of the computed `upsampling_smoothing` kernel.
    pub(crate) fft_truncation: usize,
    /// Number of pyramid layers.
    pub(crate) number_of_layers: usize,
    /// Applies denoise processing if true.
    pub(crate) denoise: bool,
    /// Layer denoise thresholds (if `denoise` is given, then provide `number_of_layers` values in descending order, starting from apex layer).
    pub(crate) denoise_thresholds: Vec<Real>,
    /// Applies enhance processing if true.
    pub(crate) enhance: bool,
    /// Layer enhance thresholds (if `enhance` is given, then provide `number_of_layers` values in descending order, starting from apex layer).
    pub(crate) enhance_thresholds: Vec<Real>,
    /// Layer enhance factors (if `enhance` is given, then provide `number_of_layers` values in descending order, starting from apex layer).
    pub(crate) enhance_factors: Vec<Real>,
    /// Applies multiply processing if true.
    pub(crate) multiply: bool,
    /// Layer multiplication factors (if `multiply` is given, then provide `number_of_layers` values in descending order, starting from apex layer).
    pub(crate) multiply_factors: Vec<Real>,
    /// The underlying detector method to apply after the multiscaling smoothing has been applied.
    pub(crate) method: MultiscalingDetectorMethod,
}

/// Encapsulates the parameters specific to the Smoothing detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MultiscalingDetectorMethod {
    /// Detects events using a fixed threshold discriminator. Event lists consist of time and voltage values.
    #[serde(
        alias = "fixed_threshold_discriminator",
        alias = "FixedThresholdDiscriminator"
    )]
    FixedThresholdDiscriminator(FixedThresholdDiscriminatorParameters),
    /// Detects events using a differential threshold discriminator. Event lists consist of time and voltage values.
    #[serde(
        alias = "differential_threshold_discriminator",
        alias = "DifferentialThresholdDiscriminator"
    )]
    DifferentialThresholdDiscriminator(DifferentialThresholdDiscriminatorParameters),
    /// Detects events using a smoothed second derivative. Event lists consist of time and voltage values.
    #[serde(alias = "smoothing_detector", alias = "SmoothingDetector")]
    SmoothingDetector(SmoothingDetectorParameters),
}

/// Specifies which detector is to be used, and wraps the detector-specific options in each variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Mode {
    /// Detects events using a fixed threshold discriminator. Event lists consist of time and voltage values.
    #[serde(
        alias = "fixed_threshold_discriminator",
        alias = "FixedThresholdDiscriminator"
    )]
    FixedThresholdDiscriminator(FixedThresholdDiscriminatorParameters),
    /// Detects events using a differential threshold discriminator. Event lists consist of time and voltage values.
    #[serde(
        alias = "differential_threshold_discriminator",
        alias = "DifferentialThresholdDiscriminator"
    )]
    DifferentialThresholdDiscriminator(DifferentialThresholdDiscriminatorParameters),
    /// Detects events using a smoothed second derivative. Event lists consist of time and voltage values.
    #[serde(alias = "smoothing_detector", alias = "SmoothingDetector")]
    SmoothingDetector(SmoothingDetectorParameters),
    /// Detects events using one of the other methods after applying multiscaling pyramid smoothing. Event lists consist of time and voltage values.
    #[serde(alias = "multiscaling", alias = "Multiscaling")]
    Multiscaling(MultiscalingDetectorParameters),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_fixed_threshold() {
        let json = r#"{
            "polarity": "positive",
            "baseline": 12,
            "mode": {
                "fixed-threshold-discriminator": {
                    "threshold": 5.5,
                    "duration": 2,
                    "cool-off": 3
                }
            }
        }"#;

        let settings: DetectorSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.polarity, Polarity::Positive);
        assert_eq!(settings.baseline, 12);
        assert_eq!(
            settings.mode,
            Mode::FixedThresholdDiscriminator(FixedThresholdDiscriminatorParameters {
                threshold: 5.5,
                duration: 2,
                cool_off: 3,
            })
        );
    }

    #[test]
    fn test_deserialize_differential_threshold() {
        let json = r#"{
            "polarity": "positive",
            "baseline": 5,
            "mode": {
                "differential-threshold-discriminator": {
                    "begin-threshold": 1.2,
                    "begin-duration": 0,
                    "end-threshold": 0.8,
                    "end-duration": 0,
                    "cool-off": 0,
                    "peak-height-mode": "max-value",
                    "peak-height-basis": "trace-baseline"
                }
            }
        }"#;

        let settings: DetectorSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.polarity, Polarity::Positive);
        assert_eq!(settings.baseline, 5);
        assert_eq!(
            settings.mode,
            Mode::DifferentialThresholdDiscriminator(
                DifferentialThresholdDiscriminatorParameters {
                    begin_threshold: 1.2,
                    begin_duration: 0,
                    end_threshold: 0.8,
                    end_duration: 0,
                    cool_off: 0,
                    peak_height_mode: PeakHeightMode::MaxValue,
                    peak_height_basis: PeakHeightBasis::TraceBaseline,
                }
            )
        );
    }

    #[test]
    fn test_deserialize_smoothing_detector() {
        let json = r#"{
            "polarity": "positive",
            "mode": {
                "smoothing-detector": {
                    "noise-centile": 0.5,
                    "kernel-sigma": 1.5,
                    "nsig-noise": 3.0,
                    "min-size": 4
                }
            }
        }"#;

        let settings: DetectorSettings = serde_json::from_str(json).unwrap();
        assert_eq!(
            settings.mode,
            Mode::SmoothingDetector(SmoothingDetectorParameters {
                noise_centile: 0.5,
                kernel_sigma: 1.5,
                nsig_noise: 3.0,
                min_size: Some(4),
                use_local_for_sizes_ge: None,
            })
        );
    }

    #[test]
    fn test_deserialize_multiscaling() {
        let json = r#"{
            "polarity": "positive",
            "mode": {
                "multiscaling": {
                    "number-of-layers": 4,
                    "denoise": true,
                    "method": {
                        "fixed-threshold-discriminator": {
                            "threshold": 7.0,
                            "duration": 1,
                            "cool-off": 0
                        }
                    }
                }
            }
        }"#;

        let settings: DetectorSettings = serde_json::from_str(json).unwrap();
        match settings.mode {
            Mode::Multiscaling(params) => {
                assert_eq!(params.number_of_layers, 4);
                assert!(params.denoise);
                assert_eq!(
                    params.method,
                    MultiscalingDetectorMethod::FixedThresholdDiscriminator(
                        FixedThresholdDiscriminatorParameters {
                            threshold: 7.0,
                            duration: 1,
                            cool_off: 0,
                        }
                    )
                );
            }
            _ => panic!("Expected multiscaling mode"),
        }
    }
}
