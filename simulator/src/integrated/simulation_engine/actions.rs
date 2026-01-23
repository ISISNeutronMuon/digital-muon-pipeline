use crate::integrated::simulation_elements::{
    Interval,
    run_messages::{SendAlarm, SendRunLogData, SendRunStart, SendRunStop, SendSampleEnvLog},
    utils::NumConstant,
};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SelectionModeOptions {
    PopFront,
    ReplaceRandom,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "source")]
pub(crate) enum SourceOptions {
    NoSource,
    SelectFromCache(SelectionModeOptions),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SendAggregatedEventListOptions {
    pub(crate) source_options: SourceOptions,
    pub(crate) channel_indices: Interval<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SendDigitiserEventListOptions(pub(crate) SourceOptions);

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SendTraceOptions(pub(crate) SelectionModeOptions);

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct GenerateTrace {
    pub(crate) event_list_index: usize,
    pub(crate) repeat: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct GenerateEventList {
    pub(crate) event_list_index: usize,
    pub(crate) repeat: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Loop<A> {
    pub(crate) start: NumConstant<usize>,
    pub(crate) end: NumConstant<usize>,
    pub(crate) schedule: Vec<A>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Timestamp {
    Now,
    AdvanceByMs(usize),
    RewindByMs(usize),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TracingLevel {
    Info,
    Debug,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct TracingEvent {
    pub(crate) level: TracingLevel,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Action {
    Comment(#[allow(unused)] String),
    TracingEvent(TracingEvent),
    WaitMs(usize),
    EnsureDelayMs(usize),
    SendRunStart(SendRunStart),
    SendRunStop(SendRunStop),
    SendRunLogData(SendRunLogData),
    SendSampleEnvLog(SendSampleEnvLog),
    SendAlarm(SendAlarm),
    //
    FrameLoop(Loop<FrameAction>),
    //
    SetTimestamp(Timestamp),
    SetVetoFlags(u16),
    SetPeriod(u64),
    SetProtonsPerPulse(u8),
    SetRunning(bool),
    //
    GenerateTrace(GenerateTrace),
    GenerateEventList(GenerateEventList),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FrameAction {
    Comment(#[allow(unused)] String),
    WaitMs(usize),
    EnsureDelayMs(usize),
    TracingEvent(TracingEvent),
    //
    SendAggregatedFrameEventList(SendAggregatedEventListOptions),
    //
    DigitiserLoop(Loop<DigitiserAction>),
    //
    SetTimestamp(Timestamp),
    //
    GenerateTrace(GenerateTrace),
    GenerateEventList(GenerateEventList),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DigitiserAction {
    Comment(#[allow(unused)] String),
    WaitMs(usize),
    EnsureDelayMs(usize),
    TracingEvent(TracingEvent),
    //
    SendDigitiserTrace(SendTraceOptions),
    SendDigitiserEventList(SendDigitiserEventListOptions),
    //
    GenerateTrace(GenerateTrace),
    GenerateEventList(GenerateEventList),
}
