//! This module encapsulates all metric functionality.
//!
//! To create a new metric, begin by following the instructions in [crate::metric::metrics::wrapper].
//! When creating the new submodule, e.g. `new_metric.rs`, the following steps are needed:
//! - New Type `Json[NewMetric]` implementing [JsonMetric].
//! - If necessary, new type `[NewMetric]Settings`.
//! - New Type `Flat[NewMetric]` implementing [FlatMetric].
//! - New Type `Partial[NewMetric]` implementing [PartialMetric].
//! - New Type `Complete[NewMetric]` implementing [CompleteMetric].
//! - New Enum `[NewMetric]Property`.
mod event_count;
mod false_count;
mod muon_lifetime;
mod pulse_height_spectra;
mod pulse_pair_time_diff;
mod wrapper;

pub(crate) use wrapper::{ListOfMetrics, MetricWrapper, MetricWrapperWithTag};

pub(crate) mod json {
    pub(crate) use super::event_count::DependentEventCount as EventCount;
    pub(crate) use super::false_count::DependentFalseCount as FalseCount;
    pub(crate) use super::muon_lifetime::DependentMuonLifetime as MuonLifetime;
    pub(crate) use super::pulse_height_spectra::DependentPulseHeightSpectra as PulseHeightSpectra;
    pub(crate) use super::pulse_pair_time_diff::DependentPulsePairTimeDiff as PulsePairTimeDiff;
}

pub(crate) mod flat {
    pub(crate) use super::event_count::ResolvedEventCount as EventCount;
    pub(crate) use super::false_count::ResolvedFalseCount as FalseCount;
    pub(crate) use super::muon_lifetime::ResolvedMuonLifetime as MuonLifetime;
    pub(crate) use super::pulse_height_spectra::ResolvedPulseHeightSpectra as PulseHeightSpectra;
    pub(crate) use super::pulse_pair_time_diff::ResolvedPulsePairTimeDiff as PulsePairTimeDiff;
}

pub(crate) mod partial {
    pub(crate) use super::event_count::PartialEventCount as EventCount;
    pub(crate) use super::false_count::PartialFalseCount as FalseCount;
    pub(crate) use super::muon_lifetime::PartialMuonLifetime as MuonLifetime;
    pub(crate) use super::pulse_height_spectra::PartialPulseHeightSpectra as PulseHeightSpectra;
    pub(crate) use super::pulse_pair_time_diff::PartialPulsePairTimeDiff as PulsePairTimeDiff;
}

pub(crate) mod complete {
    pub(crate) use super::event_count::CompletedEventCount as EventCount;
    pub(crate) use super::false_count::CompletedFalseCount as FalseCount;
    pub(crate) use super::muon_lifetime::CompletedMuonLifetime as MuonLifetime;
    pub(crate) use super::pulse_height_spectra::CompletedPulseHeightSpectra as PulseHeightSpectra;
    pub(crate) use super::pulse_pair_time_diff::CompletedPulsePairTimeDiff as PulsePairTimeDiff;
}

pub(crate) mod properties {
    pub(crate) use super::event_count::EventCountProperty as EventCount;
    pub(crate) use super::false_count::FalseCountProperty as FalseCount;
    pub(crate) use super::muon_lifetime::MuonLifetimeProperty as MuonLifetime;
    pub(crate) use super::pulse_height_spectra::PulseHeightSpectraProperty as PulseHeightSpectra;
    pub(crate) use super::pulse_pair_time_diff::PulsePairTimeDiffProperty as PulsePairTimeDiff;
}
