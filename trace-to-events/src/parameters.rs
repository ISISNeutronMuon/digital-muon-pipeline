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
    #[serde(alias = "positive", alias = "Positive")]
    Positive,
    /// Detection events register as negative signals.
    #[serde(alias = "negative", alias = "Negative")]
    Negative,
}

fn default_duration_1() -> usize {
    1
}

/// Encapsulates the parameters specific to the Fixed Threshold Discriminator detector.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FixedThresholdDiscriminatorParameters {
    /// If the detector is armed, an event is registered when the trace passes this value for the given duration.
    pub(crate) threshold: Real,

    /// The duration, in samples, that the trace must exceed the threshold for.
    #[serde(default = "default_duration_1")]
    pub(crate) duration: usize,

    /// After an event is registered, the detector disarms for this many samples.
    #[serde(default, alias = "cool_off")]
    pub(crate) cool_off: usize,
}

/// Determines how the peak height is calculated.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PeakHeightMode {
    /// Take the maximum trace value between begin trigger time and end trigger time.
    #[default]
    #[serde(alias = "max_value", alias = "MaxValue")]
    MaxValue,
    /// Take the trace value at the end trigger time.
    #[serde(alias = "value_at_end_trigger", alias = "ValueAtEndTrigger")]
    ValueAtEndTrigger,
}

/// Determines the peak height baseline.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PeakHeightBasis {
    /// The peak height is relative to the trace's baseline.
    #[default]
    #[serde(alias = "trace_baseline", alias = "TraceBaseline")]
    TraceBaseline,
    /// The peak height is relative to the pulse's baseline, i.e. the trace value at the time the pulse was first detected.
    #[serde(alias = "pulse_baseline", alias = "PulseBaseline")]
    PulseBaseline,
}

/// Encapsulates the parameters specific to the Differential Threshold Discriminator detector.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct DifferentialThresholdDiscriminatorParameters {
    /// If the detector is armed, an event is registered when the trace derivative passes this value for the given duration.
    #[serde(alias = "begin_threshold")]
    pub(crate) begin_threshold: Real,

    /// The duration, in samples, that the trace derivative must exceed the begin threshold for a detection to begin.
    #[serde(default, alias = "begin_duration")]
    pub(crate) begin_duration: usize,

    /// If a detection is in progress, an event is concluded when the trace derivative passes below this value for the given duration.
    #[serde(alias = "end_threshold")]
    pub(crate) end_threshold: Real,

    /// The duration, in samples, that the trace derivative must drop below the end threshold for a detection to end.
    #[serde(default, alias = "end_duration")]
    pub(crate) end_duration: usize,

    /// After an event is registered, the detector disarms for this many samples.
    #[serde(default, alias = "cool_off")]
    pub(crate) cool_off: usize,

    /// Determines how the peak height is computed.
    #[serde(default, alias = "peak_height_mode")]
    pub(crate) peak_height_mode: PeakHeightMode,

    /// Determines how the peak height is computed.
    #[serde(default, alias = "peak_height_basis")]
    pub(crate) peak_height_basis: PeakHeightBasis,
}

/// Encapsulates the parameters specific to the Smoothing detector.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SmoothingDetectorParameters {
    /// Centile of x to use for noise estimation.
    #[serde(alias = "noise_centile")]
    pub(crate) noise_centile: Real,
    /// Sigma of the Gaussian kernel for smoothing.
    #[serde(alias = "kernel_sigma")]
    pub(crate) kernel_sigma: Real,
    /// Number of standard deviations above noise to use as threshold.
    #[serde(alias = "nsig_noise")]
    pub(crate) nsig_noise: Real,
    /// Minimum size of region to consider a peak, if absent all regions are considered.
    #[serde(default, alias = "min_size")]
    pub(crate) min_size: Option<usize>,
    /// If set, then any region at or above this size will be converted to a list of local arg minima, rather than the global arg minimum.
    #[serde(default, alias = "use_local_for_sizes_ge")]
    pub(crate) use_local_for_sizes_ge: Option<usize>,
}

fn default_downsampling_smoothing() -> Vec<Real> {
    vec![0.125, 0.5, 0.75, 0.5, 0.125]
}

fn default_smoothing_support() -> Vec<i32> {
    vec![-2, -1, 0, 1, 2]
}

fn default_fft_padding() -> usize {
    200
}

fn default_fft_truncation() -> usize {
    20
}

fn default_number_of_layers() -> usize {
    4
}

fn default_true() -> bool {
    true
}

fn default_denoise_thresholds() -> Vec<Real> {
    vec![2.0, 2.0, 2.0, 2.0]
}

fn default_enhance_thresholds() -> Vec<Real> {
    vec![1.0, 1.0, 1.0, 1.0]
}

fn default_enhance_factors() -> Vec<Real> {
    vec![1.1, 1.1, 1.1, 1.1]
}

/// Encapsulates the parameters specific to the Multiscaling detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct MultiscalingDetectorParameters {
    /// Coefficients of the smoothing kernel to apply on downsampling.
    #[serde(
        default = "default_downsampling_smoothing",
        alias = "downsampling_smoothing"
    )]
    pub(crate) downsampling_smoothing: Vec<Real>,
    /// Support of the `downsampling_smoothing` kernel used to compute the `upsampling_smoothing` kernel.
    #[serde(default = "default_smoothing_support", alias = "smoothing_support")]
    pub(crate) smoothing_support: Vec<i32>,
    /// Amount of padding to use when calculating the `upsampling_smoothing` kernel.
    #[serde(default = "default_fft_padding", alias = "fft_padding")]
    pub(crate) fft_padding: usize,
    /// Size of the computed `upsampling_smoothing` kernel.
    #[serde(default = "default_fft_truncation", alias = "fft_truncation")]
    pub(crate) fft_truncation: usize,
    /// Number of pyramid layers.
    #[serde(default = "default_number_of_layers", alias = "number_of_layers")]
    pub(crate) number_of_layers: usize,
    /// Applies denoise processing if true.
    #[serde(default = "default_true")]
    pub(crate) denoise: bool,
    /// Layer denoise thresholds (if `denoise` is given, then provide `number_of_layers` values in descending order, starting from apex layer).
    #[serde(default = "default_denoise_thresholds", alias = "denoise_thresholds")]
    pub(crate) denoise_thresholds: Vec<Real>,
    /// Applies enhance processing if true.
    #[serde(default)]
    pub(crate) enhance: bool,
    /// Layer enhance thresholds (if `enhance` is given, then provide `number_of_layers` values in descending order, starting from apex layer).
    #[serde(default = "default_enhance_thresholds", alias = "enhance_thresholds")]
    pub(crate) enhance_thresholds: Vec<Real>,
    /// Layer enhance factors (if `enhance` is given, then provide `number_of_layers` values in descending order, starting from apex layer).
    #[serde(default = "default_enhance_factors", alias = "enhance_factors")]
    pub(crate) enhance_factors: Vec<Real>,
    /// Applies multiply processing if true.
    #[serde(default)]
    pub(crate) multiply: bool,
    /// Layer multiplication factors (if `multiply` is given, then provide `number_of_layers` values in descending order, starting from apex layer).
    #[serde(default = "default_enhance_factors", alias = "multiply_factors")]
    pub(crate) multiply_factors: Vec<Real>,
    /// The underlying detector method to apply after the multiscaling smoothing has been applied.
    pub(crate) method: MultiscalingDetectorMethod,
}

impl Default for MultiscalingDetectorParameters {
    fn default() -> Self {
        Self {
            downsampling_smoothing: default_downsampling_smoothing(),
            smoothing_support: default_smoothing_support(),
            fft_padding: default_fft_padding(),
            fft_truncation: default_fft_truncation(),
            number_of_layers: default_number_of_layers(),
            denoise: default_true(),
            denoise_thresholds: default_denoise_thresholds(),
            enhance: false,
            enhance_thresholds: default_enhance_thresholds(),
            enhance_factors: default_enhance_factors(),
            multiply: false,
            multiply_factors: default_enhance_factors(),
            method: MultiscalingDetectorMethod::default(),
        }
    }
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

impl Default for MultiscalingDetectorMethod {
    fn default() -> Self {
        Self::FixedThresholdDiscriminator(FixedThresholdDiscriminatorParameters::default())
    }
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
    fn test_deserialize_fixed_threshold_defaults() {
        let json = r#"{
            "polarity": "negative",
            "mode": {
                "fixed-threshold-discriminator": {
                    "threshold": 10.0
                }
            }
        }"#;

        let settings: DetectorSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.polarity, Polarity::Negative);
        assert_eq!(settings.baseline, 0);
        assert_eq!(
            settings.mode,
            Mode::FixedThresholdDiscriminator(FixedThresholdDiscriminatorParameters {
                threshold: 10.0,
                duration: 1,
                cool_off: 0,
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
                    "end-threshold": 0.8
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
                    "method": {
                        "fixed-threshold-discriminator": {
                            "threshold": 7.0
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

    #[test]
    fn test_deserialize_snake_case_aliases() {
        let json = r#"{
            "polarity": "positive",
            "mode": {
                "fixed_threshold_discriminator": {
                    "threshold": 4.0,
                    "duration": 3,
                    "cool_off": 1
                }
            }
        }"#;

        let settings: DetectorSettings = serde_json::from_str(json).unwrap();
        assert_eq!(
            settings.mode,
            Mode::FixedThresholdDiscriminator(FixedThresholdDiscriminatorParameters {
                threshold: 4.0,
                duration: 3,
                cool_off: 1,
            })
        );
    }
}
