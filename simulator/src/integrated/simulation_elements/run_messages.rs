use crate::{
    integrated::simulation_elements::{noise::NoiseSource, utils::TextConstant},
    runs::{
        alarm::SeverityLevel,
        runlog::ValueType,
        sample_environment::{LocationType, ValuesType},
    },
};
use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Defines the fields of a runstart message.
///
/// Should only be created via JSON deserialisation.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "run-command")]
pub(crate) struct SendRunStart {
    /// Name of the run.
    pub(crate) name: TextConstant,
    /// Name of file in which to save the NeXus data.
    pub(crate) filename: TextConstant,
    /// Name of the detector instrument.
    pub(crate) instrument: TextConstant,
}

/// Defines the fields of a runstop message.
///
/// Should only be created via JSON deserialisation.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "run-command")]
pub(crate) struct SendRunStop {
    /// Name of the run.
    ///
    /// Should match that name in the corresponding [SendRunStart].
    pub(crate) name: TextConstant,
}

/// Defines the fields of a runlog message.
///
/// Should only be created via JSON deserialisation.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "run-command")]
pub(crate) struct SendRunLogData {
    /// Name of the source of the runlog.
    pub(crate) source_name: TextConstant,
    /// Datatype of the log.
    pub(crate) value_type: ValueType,
    /// List of values of the log.
    pub(crate) value: Vec<String>,
}

/// Defines the fields of a selog message.
///
/// Should only be created via JSON deserialisation.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SendSampleEnvLog {
    /// Name of the sample environment log.
    pub(crate) name: TextConstant,
    /// Optional channel to associate the selog with.
    pub(crate) channel: Option<i32>,
    /// Optional value denoting the interval between each value in the selog.
    pub(crate) time_delta: Option<f64>,
    /// Datatype of the selog.
    pub(crate) values_type: ValuesType,
    /// Optional counter value to associate the selog with.
    pub(crate) message_counter: Option<i64>,
    /// Which part of the timebins the timestamps refer to.
    pub(crate) location: LocationType,
    /// Optional vector of timestamps to attach to the message.
    pub(crate) timestamps: Option<Vec<DateTime<Utc>>>,
    /// The values of the selog.
    pub(crate) values: SendSampleEnvLogValues,
}

/// Defines the values of a selog message.
///
/// Should only be created via JSON deserialisation.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SendSampleEnvLogValues {
    /// Vector of literal values.
    Literal(Vec<String>),
    /// Values generated from noise.
    FromNoise {
        /// Length of the resulting sample environment values.
        length: usize,
        /// List of [NoiseSource] object which generate the values.
        noise: Vec<NoiseSource>,
    },
}

/// Defines the fields of an alarm message.
///
/// Should only be created via JSON deserialisation.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "run-command")]
pub(crate) struct SendAlarm {
    /// Name of the source of the alarm.
    pub(crate) source_name: TextConstant,
    /// Level of severity to attach to the alarm.
    pub(crate) severity: SeverityLevel,
    /// Message attached to the alarm.
    pub(crate) message: String,
}
