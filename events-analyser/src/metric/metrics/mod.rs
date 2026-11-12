use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

mod event_count;
mod false_count;
mod muon_lifetime;
mod pulse_height_spectra;
mod pulse_pair_time_diff;

pub(crate) mod json {
    pub(crate) use super::event_count::JsonEventCount as EventCount;
    pub(crate) use super::false_count::JsonFalseCount as FalseCount;
    pub(crate) use super::muon_lifetime::JsonMuonLifetime as MuonLifetime;
    pub(crate) use super::pulse_height_spectra::JsonPulseHeightSpectra as PulseHeightSpectra;
    pub(crate) use super::pulse_pair_time_diff::JsonPulsePairTimeDiff as PulsePairTimeDiff;
}

pub(crate) mod flat {
    pub(crate) use super::event_count::FlatEventCount as EventCount;
    pub(crate) use super::false_count::FlatFalseCount as FalseCount;
    pub(crate) use super::muon_lifetime::FlatMuonLifetime as MuonLifetime;
    pub(crate) use super::pulse_height_spectra::FlatPulseHeightSpectra as PulseHeightSpectra;
    pub(crate) use super::pulse_pair_time_diff::FlatPulsePairTimeDiff as PulsePairTimeDiff;
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

/// Encapsulates a list of all metrics.
pub(crate) trait ListOfMetrics {
    type EventCount;
    type FalseCount;
    type MuonLifetime;
    type PulseHeightSpectra;
    type PulsePairTimeDiff;
}

/// Encapsulates an instance of references derived from `Types`.
pub(crate) struct RefMetricTypes<'a, Types : ListOfMetrics>(PhantomData<&'a Types>);

impl<'a, Types : ListOfMetrics> ListOfMetrics for RefMetricTypes<'a, Types> {
    type EventCount = &'a Types::EventCount;
    type FalseCount = &'a Types::FalseCount;
    type MuonLifetime = &'a Types::MuonLifetime;
    type PulseHeightSpectra = &'a Types::PulseHeightSpectra;
    type PulsePairTimeDiff = &'a Types::PulsePairTimeDiff;
}

/// Encapsulates an instance of mutable references derived from `Types`.
pub(crate) struct MutMetricTypes<'a, Types : ListOfMetrics>(PhantomData<&'a mut Types>);

impl<'a, Types : ListOfMetrics> ListOfMetrics for MutMetricTypes<'a, Types> {
    type EventCount = &'a mut Types::EventCount;
    type FalseCount = &'a mut Types::FalseCount;
    type MuonLifetime = &'a mut Types::MuonLifetime;
    type PulseHeightSpectra = &'a mut Types::PulseHeightSpectra;
    type PulsePairTimeDiff = &'a mut Types::PulsePairTimeDiff;
}

/// Encapsulates an instance of tuples derived from `Types1` and `Types2`.
pub(crate) struct ZippedMetricTypes<Types1 : ListOfMetrics, Types2 : ListOfMetrics>(PhantomData<(Types1, Types2)>);

impl<Types1 : ListOfMetrics, Types2 : ListOfMetrics> ListOfMetrics for ZippedMetricTypes<Types1, Types2> {
    type EventCount = (Types1::EventCount, Types2::EventCount);
    type FalseCount = (Types1::FalseCount, Types2::FalseCount);
    type MuonLifetime = (Types1::MuonLifetime, Types2::MuonLifetime);
    type PulseHeightSpectra = (Types1::PulseHeightSpectra, Types2::PulseHeightSpectra);
    type PulsePairTimeDiff = (Types1::PulsePairTimeDiff, Types2::PulsePairTimeDiff);
}

/// A generic enum in which each variant contains the corresponding type encapsulated by `Types`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "metric-type")]
pub(crate) enum MetricWrapper<Types : ListOfMetrics> {
    #[serde(rename_all = "kebab-case")]
    EventCount(Types::EventCount),
    #[serde(rename_all = "kebab-case")]
    FalseCount(Types::FalseCount),
    #[serde(rename_all = "kebab-case")]
    MuonLifetime(Types::MuonLifetime),
    #[serde(rename_all = "kebab-case")]
    PulseHeightSpectra(Types::PulseHeightSpectra),
    #[serde(rename_all = "kebab-case")]
    PulsePairTimeDiff(Types::PulsePairTimeDiff),
}

impl<Types : ListOfMetrics> MetricWrapper<Types> {
    /// Converts a reference of a wrapper into a wrapper of references.
    pub(crate) fn as_ref<'a>(&'a self) -> MetricWrapper<RefMetricTypes<'a, Types>> {
        match self {
            MetricWrapper::EventCount(this) => MetricWrapper::EventCount(this),
            MetricWrapper::FalseCount(this) => MetricWrapper::FalseCount(this),
            MetricWrapper::MuonLifetime(this) => MetricWrapper::MuonLifetime(this),
            MetricWrapper::PulseHeightSpectra(this) => MetricWrapper::PulseHeightSpectra(this),
            MetricWrapper::PulsePairTimeDiff(this) => MetricWrapper::PulsePairTimeDiff(this),
        }
    }

    /// Converts a mutable reference of a wrapper into a wrapper of mutable references.
    pub(crate) fn as_mut<'a>(&'a mut self) -> MetricWrapper<MutMetricTypes<'a, Types>> {
        match self {
            MetricWrapper::EventCount(this) => MetricWrapper::EventCount(this),
            MetricWrapper::FalseCount(this) => MetricWrapper::FalseCount(this),
            MetricWrapper::MuonLifetime(this) => MetricWrapper::MuonLifetime(this),
            MetricWrapper::PulseHeightSpectra(this) => MetricWrapper::PulseHeightSpectra(this),
            MetricWrapper::PulsePairTimeDiff(this) => MetricWrapper::PulsePairTimeDiff(this),
        }
    }

    /// Takes two wrappers and returns a wrapper of tuples.
    pub(crate) fn zip<OtherTypes : ListOfMetrics>(
        self,
        other: MetricWrapper<OtherTypes>,
    ) -> MetricWrapper<ZippedMetricTypes<Types, OtherTypes>> {
        match (self, other) {
            (Self::EventCount(this), MetricWrapper::EventCount(that)) => {
                MetricWrapper::EventCount((this, that))
            }
            (Self::FalseCount(this), MetricWrapper::FalseCount(that)) => {
                MetricWrapper::FalseCount((this, that))
            }
            (Self::MuonLifetime(this), MetricWrapper::MuonLifetime(that)) => {
                MetricWrapper::MuonLifetime((this, that))
            }
            (Self::PulseHeightSpectra(this), MetricWrapper::PulseHeightSpectra(that)) => {
                MetricWrapper::PulseHeightSpectra((this, that))
            }
            (Self::PulsePairTimeDiff(this), MetricWrapper::PulsePairTimeDiff(that)) => {
                MetricWrapper::PulsePairTimeDiff((this, that))
            }
            _ => unreachable!(),
        }
    }

    /// Converts a wrapper into a generic type, with the provided maps applied to each variant.
    /// 
    /// # Paramerers
    /// - event_count_map: function applied to the `EventCount` variant.
    /// - false_count_map: function applied to the `FalseCount` variant.
    /// - muon_lifetime_map: function applied to the `MuonLifetime` variant.
    /// - pulse_height_spectra_map: function applied to the `PulseHeightSpectra` variant.
    /// - pulse_pair_time_diff_map: function applied to the `PulsePairTimeDiff` variant.
    pub(crate) fn fold<T, FE, FF, FM, FP, FD>(self,
        event_count_map: FE,
        false_count_map: FF,
        muon_lifetime_map: FM,
        pulse_height_spectra_map: FP,
        pulse_pair_time_diff_map: FD,
    ) -> T
    where
        FE: Fn(Types::EventCount) -> T,
        FF: Fn(Types::FalseCount) -> T,
        FM: Fn(Types::MuonLifetime) -> T,
        FP: Fn(Types::PulseHeightSpectra) -> T,
        FD: Fn(Types::PulsePairTimeDiff) -> T,
    {
        match self {
            Self::EventCount(this) => event_count_map(this),
            Self::FalseCount(this) => false_count_map(this),
            Self::MuonLifetime(this) => muon_lifetime_map(this),
            Self::PulseHeightSpectra(this) => pulse_height_spectra_map(this),
            Self::PulsePairTimeDiff(this) => pulse_pair_time_diff_map(this),
        }
    }

    /// Converts one wrapper into another, with the provided maps applied to each variant.
    /// 
    /// # Paramerers
    /// - event_count_map: function applied to the `EventCount` variant.
    /// - false_count_map: function applied to the `FalseCount` variant.
    /// - muon_lifetime_map: function applied to the `MuonLifetime` variant.
    /// - pulse_height_spectra_map: function applied to the `PulseHeightSpectra` variant.
    /// - pulse_pair_time_diff_map: function applied to the `PulsePairTimeDiff` variant.
    pub(crate) fn map<OtherTypes : ListOfMetrics, FE, FF, FM, FP, FD>(
        self,
        event_count_map: FE,
        false_count_map: FF,
        muon_lifetime_map: FM,
        pulse_height_spectra_map: FP,
        pulse_pair_time_diff_map: FD,
    ) -> MetricWrapper<OtherTypes>
    where
        FE: Fn(Types::EventCount) -> OtherTypes::EventCount,
        FF: Fn(Types::FalseCount) -> OtherTypes::FalseCount,
        FM: Fn(Types::MuonLifetime) -> OtherTypes::MuonLifetime,
        FP: Fn(Types::PulseHeightSpectra) -> OtherTypes::PulseHeightSpectra,
        FD: Fn(Types::PulsePairTimeDiff) -> OtherTypes::PulsePairTimeDiff,
    {
        self.fold(
            |this| MetricWrapper::EventCount(event_count_map(this)),
            |this| MetricWrapper::FalseCount(false_count_map(this)),
            |this| MetricWrapper::MuonLifetime(muon_lifetime_map(this)),
            |this| MetricWrapper::PulseHeightSpectra(pulse_height_spectra_map(this)),
            |this| MetricWrapper::PulsePairTimeDiff(pulse_pair_time_diff_map(this)),
        )
    }

    /// Tries to convert one wrapper into another, with the provided maps applied to each variant.
    /// 
    /// # Paramerers
    /// - event_count_map: function applied to the `EventCount` variant.
    /// - false_count_map: function applied to the `FalseCount` variant.
    /// - muon_lifetime_map: function applied to the `MuonLifetime` variant.
    /// - pulse_height_spectra_map: function applied to the `PulseHeightSpectra` variant.
    /// - pulse_pair_time_diff_map: function applied to the `PulsePairTimeDiff` variant.
    pub(crate) fn try_map<OtherTypes : ListOfMetrics, FE, FF, FM, FP, FD, Error, EE, EF, EM, EP, ED>(
        self,
        event_count_map: FE,
        false_count_map: FF,
        muon_lifetime_map: FM,
        pulse_height_spectra_map: FP,
        pulse_pair_time_diff_map: FD,
    ) -> Result<MetricWrapper<OtherTypes>, Error>
    where
        FE: Fn(Types::EventCount) -> Result<OtherTypes::EventCount, EE>,
        FF: Fn(Types::FalseCount) -> Result<OtherTypes::FalseCount, EF>,
        FM: Fn(Types::MuonLifetime) -> Result<OtherTypes::MuonLifetime, EM>,
        FP: Fn(Types::PulseHeightSpectra) -> Result<OtherTypes::PulseHeightSpectra, EP>,
        FD: Fn(Types::PulsePairTimeDiff) -> Result<OtherTypes::PulsePairTimeDiff, ED>,
        EE: Into<Error>,
        EF: Into<Error>,
        EM: Into<Error>,
        EP: Into<Error>,
        ED: Into<Error>
    {
        self.fold(
            |this|event_count_map(this).map(MetricWrapper::EventCount).map_err(Into::into),
            |this|false_count_map(this).map(MetricWrapper::FalseCount).map_err(Into::into),
            |this|muon_lifetime_map(this).map(MetricWrapper::MuonLifetime).map_err(Into::into),
            |this|pulse_height_spectra_map(this).map(MetricWrapper::PulseHeightSpectra).map_err(Into::into),
            |this|pulse_pair_time_diff_map(this).map(MetricWrapper::PulsePairTimeDiff).map_err(Into::into),
        )
    }
}
