//! Defines the central [MetricWrapper] generic enum type.
//! This type has one variant for each metric and is intended so that most changes
//! resulting from modifying or adding new metrics are contained withing the [crate::metric::metrics] module.
//! Any other modifications necessary from adding a new metric are indicated by compiler errors.
//!
//! To add a new metric:
//! - Add a new `type` into the trait [ListOfMetrics] (and update `Example`).
//! - Update [RefMetricTypes], [MutMetricTypes], and [ZippedMetricTypes] as needed (including `Example`).
//! - Add new module in [crate::metric::metrics] defining the necessary types as detailed in the module's documentation.
//! - Update [crate::engine::elements::metric] as needed.
//! - Update [crate::analysis::results::partial] and [crate::analysis::results::complete] as needed.
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// Types implementing this encapsulate a list of all metrics.
///
/// # Example
/// ```rust
///    // These do something with each metric.
///    struct EventCountType;
///    struct FalseCountType;
///    struct MuonLifetimeType;
///    struct PulseHeightSpectraType;
///    struct PulsePairTimeDiffType;
///    struct ClusterCountType;
///
///    // This allows all the above types to be grouped together.
///    struct MetricsList;
///
///    impl ListOfMetrics for MetricsList {
///        type EventCount = EventCountType;
///        type FalseCount = FalseCountType;
///        type MuonLifetime = MuonLifetimeType;
///        type PulseHeightSpectra = PulseHeightSpectraType;
///        type PulsePairTimeDiff = PulsePairTimeDiffType;
///        type ClusterCount = ClusterCountType;
///    }
/// ```
///
/// For an example of how this is used see [MetricWrapper].
pub(crate) trait ListOfMetrics {
    type EventCount;
    type FalseCount;
    type MuonLifetime;
    type PulseHeightSpectra;
    type PulsePairTimeDiff;
    type ClusterCount;
}

/// Creates an impl block implementing trait [ListOfMetrics] for an empty type.
/// For each associated type a type of the same name, within a module should be provided.
///
/// # Example
///
/// ```rust
///     mod my_module {
///         struct EventCount;
///         struct FalseCount;
///         struct MuonLifetime;
///         struct PulseHeightSpectra;
///         struct PulsePairTimeDiff;
///         struct ClusterCount;
///     }
///     struct MetricsList;
///     impl_list_of_metrics!(MetricsList => my_module)
/// ```
/// expands to
/// ```rust
///     impl ListOfMetrics for MetricsList {
///         type EventCount = my_module::EventCount;
///         type FalseCount = my_module::FalseCount;
///         type MuonLifetime = my_module::MuonLifetime;
///         type PulseHeightSpectra = my_module::PulseHeightSpectra;
///         type PulsePairTimeDiff = my_module::PulsePairTimeDiff;
///         type ClusterCount = my_module::ClusterCount;
///     }
/// ```
///
/// The module can be wrapped in a type as a parameter, for instance:
/// ```rust
///     struct MyType<T>;
///     impl_list_of_metrics!(MetricsList => MyType<my_module>)
/// ```
/// expands to
/// ```rust
///     impl ListOfMetrics for MetricsList {
///         type EventCount = MyType<my_module::EventCount>;
///         type FalseCount = MyType<my_module::FalseCount>;
///         type MuonLifetime = MyType<my_module::MuonLifetime>;
///         type PulseHeightSpectra = MyType<my_module::PulseHeightSpectra>;
///         type PulsePairTimeDiff = MyType<my_module::PulsePairTimeDiff>;
///         type ClusterCount = MyType<my_module::ClusterCount>;
///     }
/// ```
#[macro_export]
macro_rules! impl_list_of_metrics {
    ($type:ty => $module:ident) => {
        impl ListOfMetrics for $type {
            type EventCount = $module::EventCount;
            type FalseCount = $module::FalseCount;
            type MuonLifetime = $module::MuonLifetime;
            type PulseHeightSpectra = $module::PulseHeightSpectra;
            type PulsePairTimeDiff = $module::PulsePairTimeDiff;
            type ClusterCount = $module::ClusterCount;
        }
    };
    ($type:ty => $prefix:ident<$module:ident>) => {
        impl ListOfMetrics for $type {
            type EventCount = $prefix<$module::EventCount>;
            type FalseCount = $prefix<$module::FalseCount>;
            type MuonLifetime = $prefix<$module::MuonLifetime>;
            type PulseHeightSpectra = $prefix<$module::PulseHeightSpectra>;
            type PulsePairTimeDiff = $prefix<$module::PulsePairTimeDiff>;
            type ClusterCount = $prefix<$module::ClusterCount>;
        }
    };
}

/// Generic enum in which each variant contains the corresponding type encapsulated by `Types`.
///
/// # Example
///
/// Consider the types defined in the [ListOfMetrics] example.
///
/// ```rust
///    struct EventCountType;
///    struct FalseCountType;
///    struct MuonLifetimeType;
///    struct PulseHeightSpectraType;
///    struct PulsePairTimeDiffType;
///    struct ClusterCountType;
///
///    struct MetricsList;
///
///    impl ListOfMetrics for MetricsList {
///        type EventCount = EventCountType;
///        type FalseCount = FalseCountType;
///        type MuonLifetime = MuonLifetimeType;
///        type PulseHeightSpectra = PulseHeightSpectraType;
///        type PulsePairTimeDiff = PulsePairTimeDiffType;
///        type ClusterCount = ClusterCountType;
///    }
///
///     // This does something different depening on the type of metric.
///     type MetricType = MetricWrapper<MetricsList>;
/// ```
///
/// For an example of how this is used see [MetricWrapper].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MetricWrapper<Types: ListOfMetrics> {
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
    #[serde(rename_all = "kebab-case")]
    ClusterCount(Types::ClusterCount),
}

/// Encapsulates an instance of references derived from `Types`.
/// [MetricWrapper] instances are able to call [MetricWrapper::as_ref] to obtain
/// an instance of [RefMetricTypes].
pub(crate) struct RefMetricTypes<'a, Types: ListOfMetrics>(PhantomData<&'a Types>);

impl<'a, Types: ListOfMetrics> ListOfMetrics for RefMetricTypes<'a, Types> {
    type EventCount = &'a Types::EventCount;
    type FalseCount = &'a Types::FalseCount;
    type MuonLifetime = &'a Types::MuonLifetime;
    type PulseHeightSpectra = &'a Types::PulseHeightSpectra;
    type PulsePairTimeDiff = &'a Types::PulsePairTimeDiff;
    type ClusterCount = &'a Types::ClusterCount;
}

/// Encapsulates an instance of mutable references derived from `Types`.
/// [MetricWrapper] instances are able to call [MetricWrapper::as_mut] to obtain
/// an instance of [MutMetricTypes].
pub(crate) struct MutMetricTypes<'a, Types: ListOfMetrics>(PhantomData<&'a mut Types>);

impl<'a, Types: ListOfMetrics> ListOfMetrics for MutMetricTypes<'a, Types> {
    type EventCount = &'a mut Types::EventCount;
    type FalseCount = &'a mut Types::FalseCount;
    type MuonLifetime = &'a mut Types::MuonLifetime;
    type PulseHeightSpectra = &'a mut Types::PulseHeightSpectra;
    type PulsePairTimeDiff = &'a mut Types::PulsePairTimeDiff;
    type ClusterCount = &'a mut Types::ClusterCount;
}

/// Encapsulates an instance of tuples derived from `Types1` and `Types2`.
/// [MetricWrapper] instances are able to call [MetricWrapper::zip] on another
/// [MetricWrapper] instance to obtain an instance of [ZippedMetricTypes].
pub(crate) struct ZippedMetricTypes<Types1: ListOfMetrics, Types2: ListOfMetrics>(
    PhantomData<(Types1, Types2)>,
);

impl<Types1: ListOfMetrics, Types2: ListOfMetrics> ListOfMetrics
    for ZippedMetricTypes<Types1, Types2>
{
    type EventCount = (Types1::EventCount, Types2::EventCount);
    type FalseCount = (Types1::FalseCount, Types2::FalseCount);
    type MuonLifetime = (Types1::MuonLifetime, Types2::MuonLifetime);
    type PulseHeightSpectra = (Types1::PulseHeightSpectra, Types2::PulseHeightSpectra);
    type PulsePairTimeDiff = (Types1::PulsePairTimeDiff, Types2::PulsePairTimeDiff);
    type ClusterCount = (Types1::ClusterCount, Types2::ClusterCount);
}

impl<Types: ListOfMetrics> MetricWrapper<Types> {
    /// Converts a reference of a wrapper into a wrapper of references.
    pub(crate) fn as_ref<'a>(&'a self) -> MetricWrapper<RefMetricTypes<'a, Types>> {
        match self {
            MetricWrapper::EventCount(this) => MetricWrapper::EventCount(this),
            MetricWrapper::FalseCount(this) => MetricWrapper::FalseCount(this),
            MetricWrapper::MuonLifetime(this) => MetricWrapper::MuonLifetime(this),
            MetricWrapper::PulseHeightSpectra(this) => MetricWrapper::PulseHeightSpectra(this),
            MetricWrapper::PulsePairTimeDiff(this) => MetricWrapper::PulsePairTimeDiff(this),
            MetricWrapper::ClusterCount(this) => MetricWrapper::ClusterCount(this),
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
            MetricWrapper::ClusterCount(this) => MetricWrapper::ClusterCount(this),
        }
    }

    /// Takes two wrappers (with the same variants) and returns a wrapper of tuples.
    ///
    /// Returns [Option::None], if the two wrappers are of different variants.
    pub(crate) fn zip<OtherTypes: ListOfMetrics>(
        self,
        other: MetricWrapper<OtherTypes>,
    ) -> Option<MetricWrapper<ZippedMetricTypes<Types, OtherTypes>>> {
        Some(match (self, other) {
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
            (Self::ClusterCount(this), MetricWrapper::ClusterCount(that)) => {
                MetricWrapper::ClusterCount((this, that))
            }
            _ => return None,
        })
    }

    /// Converts a [MetricWrapper] instance into a generic type, with the provided maps applied to each variant.
    ///
    /// # Paramerers
    /// - event_count_map: function applied to the `EventCount` variant.
    /// - false_count_map: function applied to the `FalseCount` variant.
    /// - muon_lifetime_map: function applied to the `MuonLifetime` variant.
    /// - pulse_height_spectra_map: function applied to the `PulseHeightSpectra` variant.
    /// - pulse_pair_time_diff_map: function applied to the `PulsePairTimeDiff` variant.
    /// - cluster_count_map: function applied to the `ClusterCount` variant.
    pub(crate) fn fold<
        T,
        FE: Fn(Types::EventCount) -> T,
        FF: Fn(Types::FalseCount) -> T,
        FM: Fn(Types::MuonLifetime) -> T,
        FP: Fn(Types::PulseHeightSpectra) -> T,
        FD: Fn(Types::PulsePairTimeDiff) -> T,
        FC: Fn(Types::ClusterCount) -> T,
    >(
        self,
        event_count_map: FE,
        false_count_map: FF,
        muon_lifetime_map: FM,
        pulse_height_spectra_map: FP,
        pulse_pair_time_diff_map: FD,
        cluster_count_map: FC,
    ) -> T {
        match self {
            Self::EventCount(this) => event_count_map(this),
            Self::FalseCount(this) => false_count_map(this),
            Self::MuonLifetime(this) => muon_lifetime_map(this),
            Self::PulseHeightSpectra(this) => pulse_height_spectra_map(this),
            Self::PulsePairTimeDiff(this) => pulse_pair_time_diff_map(this),
            Self::ClusterCount(this) => cluster_count_map(this),
        }
    }

    /// Converts one [MetricWrapper] instance into another, with the provided maps applied to each variant.
    ///
    /// # Paramerers
    /// - event_count_map: function applied to the `EventCount` variant.
    /// - false_count_map: function applied to the `FalseCount` variant.
    /// - muon_lifetime_map: function applied to the `MuonLifetime` variant.
    /// - pulse_height_spectra_map: function applied to the `PulseHeightSpectra` variant.
    /// - pulse_pair_time_diff_map: function applied to the `PulsePairTimeDiff` variant.
    /// - cluster_count_map: function applied to the `ClusterCount` variant.
    pub(crate) fn map<
        OtherTypes: ListOfMetrics,
        FE: Fn(Types::EventCount) -> OtherTypes::EventCount,
        FF: Fn(Types::FalseCount) -> OtherTypes::FalseCount,
        FM: Fn(Types::MuonLifetime) -> OtherTypes::MuonLifetime,
        FP: Fn(Types::PulseHeightSpectra) -> OtherTypes::PulseHeightSpectra,
        FD: Fn(Types::PulsePairTimeDiff) -> OtherTypes::PulsePairTimeDiff,
        FC: Fn(Types::ClusterCount) -> OtherTypes::ClusterCount,
    >(
        self,
        event_count_map: FE,
        false_count_map: FF,
        muon_lifetime_map: FM,
        pulse_height_spectra_map: FP,
        pulse_pair_time_diff_map: FD,
        cluster_count_map: FC,
    ) -> MetricWrapper<OtherTypes> {
        self.fold(
            |this| MetricWrapper::EventCount(event_count_map(this)),
            |this| MetricWrapper::FalseCount(false_count_map(this)),
            |this| MetricWrapper::MuonLifetime(muon_lifetime_map(this)),
            |this| MetricWrapper::PulseHeightSpectra(pulse_height_spectra_map(this)),
            |this| MetricWrapper::PulsePairTimeDiff(pulse_pair_time_diff_map(this)),
            |this| MetricWrapper::ClusterCount(cluster_count_map(this)),
        )
    }

    /// Tries to convert one [MetricWrapper] instance into another, with the provided maps applied to each variant.
    ///
    /// # Paramerers
    /// - event_count_map: function applied to the `EventCount` variant.
    /// - false_count_map: function applied to the `FalseCount` variant.
    /// - muon_lifetime_map: function applied to the `MuonLifetime` variant.
    /// - pulse_height_spectra_map: function applied to the `PulseHeightSpectra` variant.
    /// - pulse_pair_time_diff_map: function applied to the `PulsePairTimeDiff` variant.
    /// - cluster_count_map: function applied to the `ClusterCount` variant.
    pub(crate) fn try_map<
        OtherTypes: ListOfMetrics,
        FE: Fn(Types::EventCount) -> Result<OtherTypes::EventCount, EE>,
        FF: Fn(Types::FalseCount) -> Result<OtherTypes::FalseCount, EF>,
        FM: Fn(Types::MuonLifetime) -> Result<OtherTypes::MuonLifetime, EM>,
        FP: Fn(Types::PulseHeightSpectra) -> Result<OtherTypes::PulseHeightSpectra, EP>,
        FD: Fn(Types::PulsePairTimeDiff) -> Result<OtherTypes::PulsePairTimeDiff, ED>,
        FC: Fn(Types::ClusterCount) -> Result<OtherTypes::ClusterCount, EC>,
        Error,
        EE: Into<Error>,
        EF: Into<Error>,
        EM: Into<Error>,
        EP: Into<Error>,
        ED: Into<Error>,
        EC: Into<Error>,
    >(
        self,
        event_count_map: FE,
        false_count_map: FF,
        muon_lifetime_map: FM,
        pulse_height_spectra_map: FP,
        pulse_pair_time_diff_map: FD,
        cluster_count_map: FC,
    ) -> Result<MetricWrapper<OtherTypes>, Error> {
        self.fold(
            |this| {
                event_count_map(this)
                    .map(MetricWrapper::EventCount)
                    .map_err(Into::into)
            },
            |this| {
                false_count_map(this)
                    .map(MetricWrapper::FalseCount)
                    .map_err(Into::into)
            },
            |this| {
                muon_lifetime_map(this)
                    .map(MetricWrapper::MuonLifetime)
                    .map_err(Into::into)
            },
            |this| {
                pulse_height_spectra_map(this)
                    .map(MetricWrapper::PulseHeightSpectra)
                    .map_err(Into::into)
            },
            |this| {
                pulse_pair_time_diff_map(this)
                    .map(MetricWrapper::PulsePairTimeDiff)
                    .map_err(Into::into)
            },
            |this| {
                cluster_count_map(this)
                    .map(MetricWrapper::ClusterCount)
                    .map_err(Into::into)
            },
        )
    }
}

/// Generic enum in which each variant contains the corresponding type encapsulated by `Types`.
/// Identical to [MetricWrapper] except for the presence of `#[serde(tag = "metric-type")]` and
/// the `impl MetricWrapper` block.
/// This is used exclusively by [MetricType].
///
/// [MetricType] : crate::engine::elements::metric::MetricType
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "metric-type")]
pub(crate) enum MetricWrapperWithTag<Types: ListOfMetrics> {
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
    #[serde(rename_all = "kebab-case")]
    ClusterCount(Types::ClusterCount),
}

impl<T: ListOfMetrics> MetricWrapperWithTag<T> {
    /// Converts a reference of a wrapper into a wrapper of references.
    pub(crate) fn as_ref<'a>(&'a self) -> MetricWrapper<RefMetricTypes<'a, T>> {
        match self {
            MetricWrapperWithTag::EventCount(this) => MetricWrapper::EventCount(this),
            MetricWrapperWithTag::FalseCount(this) => MetricWrapper::FalseCount(this),
            MetricWrapperWithTag::MuonLifetime(this) => MetricWrapper::MuonLifetime(this),
            MetricWrapperWithTag::PulseHeightSpectra(this) => {
                MetricWrapper::PulseHeightSpectra(this)
            }
            MetricWrapperWithTag::PulsePairTimeDiff(this) => MetricWrapper::PulsePairTimeDiff(this),
            MetricWrapperWithTag::ClusterCount(this) => MetricWrapper::ClusterCount(this),
        }
    }
}
