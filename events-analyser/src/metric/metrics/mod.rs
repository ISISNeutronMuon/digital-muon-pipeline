use crate::{
    engine::{Flattenable, ValueError},
    metric::{FlatMetricShell, JsonMetricShell},
};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "metric-type")]
pub(crate) enum MetricWrapper<E, F, M, P, D> {
    #[serde(rename_all = "kebab-case")]
    EventCount(E),
    #[serde(rename_all = "kebab-case")]
    FalseCount(F),
    #[serde(rename_all = "kebab-case")]
    MuonLifetime(M),
    #[serde(rename_all = "kebab-case")]
    PulseHeightSpectra(P),
    #[serde(rename_all = "kebab-case")]
    PulsePairDifferences(D),
}

impl<E, F, M, P, D> MetricWrapper<E, F, M, P, D> {
    pub(crate) fn zip<E2, F2, M2, P2, D2>(
        self,
        other: MetricWrapper<E2, F2, M2, P2, D2>,
    ) -> MetricWrapper<(E, E2), (F, F2), (M, M2), (P, P2), (D, D2)> {
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
            (Self::PulsePairDifferences(this), MetricWrapper::PulsePairDifferences(that)) => {
                MetricWrapper::PulsePairDifferences((this, that))
            }
            _ => unreachable!(),
        }
    }

    pub(crate) fn as_ref(&self) -> MetricWrapper<&E, &F, &M, &P, &D> {
        match self {
            MetricWrapper::EventCount(this) => MetricWrapper::EventCount(this),
            MetricWrapper::FalseCount(this) => MetricWrapper::FalseCount(this),
            MetricWrapper::MuonLifetime(this) => MetricWrapper::MuonLifetime(this),
            MetricWrapper::PulseHeightSpectra(this) => MetricWrapper::PulseHeightSpectra(this),
            MetricWrapper::PulsePairDifferences(this) => MetricWrapper::PulsePairDifferences(this),
        }
    }

    pub(crate) fn as_mut(&mut self) -> MetricWrapper<&mut E, &mut F, &mut M, &mut P, &mut D> {
        match self {
            MetricWrapper::EventCount(this) => MetricWrapper::EventCount(this),
            MetricWrapper::FalseCount(this) => MetricWrapper::FalseCount(this),
            MetricWrapper::MuonLifetime(this) => MetricWrapper::MuonLifetime(this),
            MetricWrapper::PulseHeightSpectra(this) => MetricWrapper::PulseHeightSpectra(this),
            MetricWrapper::PulsePairDifferences(this) => MetricWrapper::PulsePairDifferences(this),
        }
    }

    pub(crate) fn map<E2, F2, M2, P2, D2, FE, FF, FM, FP, FD>(
        self,
        fe: FE,
        ff: FF,
        fm: FM,
        fp: FP,
        fd: FD,
    ) -> MetricWrapper<E2, F2, M2, P2, D2>
    where
        FE: Fn(E) -> E2,
        FF: Fn(F) -> F2,
        FM: Fn(M) -> M2,
        FP: Fn(P) -> P2,
        FD: Fn(D) -> D2,
    {
        self.fold(
            |this| MetricWrapper::EventCount(fe(this)),
            |this| MetricWrapper::FalseCount(ff(this)),
            |this| MetricWrapper::MuonLifetime(fm(this)),
            |this| MetricWrapper::PulseHeightSpectra(fp(this)),
            |this| MetricWrapper::PulsePairDifferences(fd(this)),
        )
    }

    pub(crate) fn fold<T, FE, FF, FM, FP, FD>(self, fe: FE, ff: FF, fm: FM, fp: FP, fd: FD) -> T
    where
        FE: Fn(E) -> T,
        FF: Fn(F) -> T,
        FM: Fn(M) -> T,
        FP: Fn(P) -> T,
        FD: Fn(D) -> T,
    {
        match self {
            MetricWrapper::EventCount(this) => fe(this),
            MetricWrapper::FalseCount(this) => ff(this),
            MetricWrapper::MuonLifetime(this) => fm(this),
            MetricWrapper::PulseHeightSpectra(this) => fp(this),
            MetricWrapper::PulsePairDifferences(this) => fd(this),
        }
    }

    pub(crate) fn try_map<E2, F2, M2, P2, D2, FnE, FnF, FnM, FnP, FnD, ERROR, EE, EF, EM, EP, ED>(
        self,
        fe: FnE,
        ff: FnF,
        fm: FnM,
        fp: FnP,
        fd: FnD,
    ) -> Result<MetricWrapper<E2, F2, M2, P2, D2>, ERROR>
    where
        FnE: Fn(E) -> Result<E2, EE>,
        FnF: Fn(F) -> Result<F2, EF>,
        FnM: Fn(M) -> Result<M2, EM>,
        FnP: Fn(P) -> Result<P2, EP>,
        FnD: Fn(D) -> Result<D2, ED>,
        ERROR: From<EE> + From<EF> + From<EM> + From<EP> + From<ED>,
    {
        Ok(match self {
            MetricWrapper::EventCount(this) => MetricWrapper::EventCount(fe(this)?),
            MetricWrapper::FalseCount(this) => MetricWrapper::FalseCount(ff(this)?),
            MetricWrapper::MuonLifetime(this) => MetricWrapper::MuonLifetime(fm(this)?),
            MetricWrapper::PulseHeightSpectra(this) => MetricWrapper::PulseHeightSpectra(fp(this)?),
            MetricWrapper::PulsePairDifferences(this) => {
                MetricWrapper::PulsePairDifferences(fd(this)?)
            }
        })
    }
}

pub(crate) type MetricType = MetricWrapper<
    JsonMetricShell<json::EventCount>,
    JsonMetricShell<json::FalseCount>,
    JsonMetricShell<json::MuonLifetime>,
    JsonMetricShell<json::PulseHeightSpectra>,
    JsonMetricShell<json::PulsePairTimeDiff>,
>;

pub(crate) type PropertyOfMetric = MetricWrapper<
    properties::EventCount,
    properties::FalseCount,
    properties::MuonLifetime,
    properties::PulseHeightSpectra,
    properties::PulsePairTimeDiff,
>;

pub(crate) type FlatMetricType = MetricWrapper<
    FlatMetricShell<flat::EventCount>,
    FlatMetricShell<flat::FalseCount>,
    FlatMetricShell<flat::MuonLifetime>,
    FlatMetricShell<flat::PulseHeightSpectra>,
    FlatMetricShell<flat::PulsePairTimeDiff>,
>;

impl Flattenable<&[String]> for MetricType {
    type Flat = FlatMetricType;
    type Error = ValueError;

    fn flatten(&self, library: &[String]) -> Result<Self::Flat, Self::Error> {
        Ok(match &self {
            MetricType::EventCount(json_metric_shell) => {
                FlatMetricType::EventCount(json_metric_shell.flatten(library)?)
            }
            MetricType::FalseCount(json_metric_shell) => {
                FlatMetricType::FalseCount(json_metric_shell.flatten(library)?)
            }
            MetricType::MuonLifetime(json_metric_shell) => {
                FlatMetricType::MuonLifetime(json_metric_shell.flatten(library)?)
            }
            MetricType::PulseHeightSpectra(json_metric_shell) => {
                FlatMetricType::PulseHeightSpectra(json_metric_shell.flatten(library)?)
            }
            MetricType::PulsePairDifferences(json_metric_shell) => {
                FlatMetricType::PulsePairDifferences(json_metric_shell.flatten(library)?)
            }
        })
    }
}
