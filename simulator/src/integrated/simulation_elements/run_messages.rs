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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "run-command")]
pub(crate) struct SendRunStart {
    pub(crate) name: TextConstant,
    pub(crate) filename: TextConstant,
    pub(crate) instrument: TextConstant,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "run-command")]
pub(crate) struct SendRunStop {
    pub(crate) name: TextConstant,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "run-command")]
pub(crate) struct SendRunLogData {
    pub(crate) source_name: TextConstant,
    pub(crate) value_type: ValueType,
    pub(crate) value: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SendSampleEnvLog {
    pub(crate) name: TextConstant,
    pub(crate) channel: Option<i32>,
    pub(crate) time_delta: Option<f64>,
    pub(crate) values_type: ValuesType,
    pub(crate) message_counter: Option<i64>,
    pub(crate) location: LocationType,
    pub(crate) timestamps: Option<Vec<DateTime<Utc>>>,
    pub(crate) values: SendSampleEnvLogValues,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SendSampleEnvLogValues {
    Literal(Vec<String>),
    FromNoise {
        length: usize,
        noise: Vec<NoiseSource>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "run-command")]
pub(crate) struct SendAlarm {
    pub(crate) source_name: TextConstant,
    pub(crate) severity: SeverityLevel,
    pub(crate) message: String,
}
