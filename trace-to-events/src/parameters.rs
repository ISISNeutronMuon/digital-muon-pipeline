//! Defines the parameters used by the various detectors defined in this component.
use crate::pulse_detection::Real;
use clap::{Parser, Subcommand, ValueEnum};
use digital_muon_common::Intensity;

#[derive(Debug)]
pub(crate) struct DetectorSettings<'a> {
    /// The type of detector to use.
    pub(crate) mode: &'a Mode,
    /// The polarity of the trace signal.
    pub(crate) polarity: &'a Polarity,
    /// The baseline of the trace signal.
    pub(crate) baseline: Intensity,
}

/// Defines the polarity of the signal, i.e. whether events cause positive or negative signals.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum Polarity {
    /// Detection events register as positive signals.
    Positive,
    /// Detection events register as negative signals.
    Negative,
}

/// Encapsulates the parameters specific to the Fixed Threshold Discriminator detector.
#[derive(Default, Debug, Clone, Parser)]
pub(crate) struct FixedThresholdDiscriminatorParameters {
    /// If the detector is armed, an event is registered when the trace passes this value for the given duration.
    #[clap(long)]
    pub(crate) threshold: Real,

    /// The duration, in samples, that the trace must exceed the threshold for.
    #[clap(long, default_value = "1")]
    pub(crate) duration: i32,

    /// After an event is registered, the detector disarms for this many samples.
    #[clap(long, default_value = "0")]
    pub(crate) cool_off: i32,
}

/// Determines how the peak height is calculated.
#[derive(Default, Debug, Clone, ValueEnum)]
pub(crate) enum PeakHeightMode {
    /// Take the maximum trace value between begin trigger time and end trigger time.
    #[default]
    MaxValue,
    /// Take the trace value at the end trigger time.
    ValueAtEndTrigger,
}

/// Determines the peak height baseline.
#[derive(Default, Debug, Clone, ValueEnum)]
pub(crate) enum PeakHeightBasis {
    /// The peak height is relative to the trace's baseline.
    #[default]
    TraceBaseline,
    /// The peak height is relative to the pulse's baseline, i.e. the trace value at the time the pulse was first detected.
    PulseBaseline,
}

/// Encapsulates the parameters specific to the Differential Threshold Discriminator detector.
#[derive(Default, Debug, Clone, Parser)]
pub(crate) struct DifferentialThresholdDiscriminatorParameters {
    /// If the detector is armed, an event is registered when the trace derivative passes this value for the given duration.
    #[clap(long)]
    pub(crate) begin_threshold: Real,

    /// The duration, in samples, that the trace derivative must exceed the begin threshold for a detection to begin.
    #[clap(long, default_value = "0")]
    pub(crate) begin_duration: i32,

    /// If a detection is in progress, an event is concluded when the trace derivative passes below this value for the given duration.
    #[clap(long)]
    pub(crate) end_threshold: Real,

    /// The duration, in samples, that the trace derivative must drop below the end threshold for a detection to end.
    #[clap(long, default_value = "0")]
    pub(crate) end_duration: i32,

    /// After an event is registered, the detector disarms for this many samples.
    #[clap(long, default_value = "0")]
    pub(crate) cool_off: i32,

    /// Determines how the peak height is computed.
    #[clap(long)]
    pub(crate) peak_height_mode: PeakHeightMode,

    /// Determines how the peak height is computed.
    #[clap(long)]
    pub(crate) peak_height_basis: PeakHeightBasis,
}

/// Encapsulates the parameters specific to the Advanced Muon detector.
#[derive(Default, Debug, Clone, Parser)]
pub(crate) struct AdvancedMuonDetectorParameters {
    /// Differential threshold for detecting muon onset. See README.md.
    #[clap(long)]
    pub(crate) muon_onset: Real,

    /// Differential threshold for detecting muon peak. See README.md.
    #[clap(long)]
    pub(crate) muon_fall: Real,

    /// Differential threshold for detecting muon termination. See README.md.
    #[clap(long)]
    pub(crate) muon_termination: Real,

    /// Length of time a threshold must be passed to register. See README.md.
    #[clap(long)]
    pub(crate) duration: Real,

    /// Size of initial portion of the trace to use for determining the baseline. Initial portion should be event free.
    #[clap(long)]
    pub(crate) baseline_length: Option<usize>,

    /// Size of the moving average window to use for the lopass filter.
    #[clap(long)]
    pub(crate) smoothing_window_size: Option<usize>,

    /// If set, filters out events whose peak is greater than the given value.
    #[clap(long)]
    pub(crate) max_amplitude: Option<Real>,

    /// If set, filters out events whose peak is less than the given value.
    #[clap(long)]
    pub(crate) min_amplitude: Option<Real>,
}

/// Specifies which detector is to be used, and wraps the detector-specific options in each variant.
#[derive(Subcommand, Debug)]
pub(crate) enum Mode {
    /// Detects events using a fixed threshold discriminator. Event lists consist of time and voltage values.
    FixedThresholdDiscriminator(FixedThresholdDiscriminatorParameters),
    /// Detects events using a differential threshold discriminator. Event lists consist of time and voltage values.
    DifferentialThresholdDiscriminator(DifferentialThresholdDiscriminatorParameters),
    /// Detects events using differential discriminators. Event lists consist of time and voltage values.
    AdvancedMuonDetector(AdvancedMuonDetectorParameters),
}
