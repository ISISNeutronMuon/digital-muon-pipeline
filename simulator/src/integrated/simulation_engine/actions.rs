use crate::integrated::simulation_elements::{
    Interval,
    run_messages::{SendAlarm, SendRunLogData, SendRunStart, SendRunStop, SendSampleEnvLog},
    utils::NumConstant,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Determines how an element of a [Cache] is selected.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SelectionModeOptions {
    /// Take the element at the front of the cache and remove it.
    PopFront,
    /// Take a random element in the cache but do not remove it.
    ReplaceRandom,
}

/// Whether to select elements from a [Cache] or not.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "source")]
pub(crate) enum SourceOptions {
    /// Do not select an element, i.e. for an empty message.
    NoSource,
    /// Select from a cache using the provided selection mode.
    SelectFromCache(SelectionModeOptions),
}

/// Send a frame eventlist message.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SendAggregatedEventListOptions {
    /// Determine how to select the eventlists.
    pub(crate) source_options: SourceOptions,
    /// Include the following range of channels.
    /// 
    /// These are indices into the channel list of the [SimulationEngine].
    pub(crate) channel_indices: Interval<usize>,
}

/// Send a digitiser eventlist message.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SendDigitiserEventListOptions(pub(crate) SourceOptions);

/// Send a digitiser trace message.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SendTraceOptions(pub(crate) SelectionModeOptions);

/// Generate one or several traces, to be placed into the trace cache.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct GenerateTrace {
    /// Event list template to generate the trace from.
    pub(crate) event_list_index: usize,
    /// Number of traces to create.
    pub(crate) repeat: usize,
}

/// Generate one or several eventlists, to be placed into the eventlist cache.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct GenerateEventList {
    /// Event list template to generate the eventlists from.
    pub(crate) event_list_index: usize,
    /// Number of eventlists to create.
    pub(crate) repeat: usize,
}

/// Create a loop of actions `A` where `A = DigitiserAction`, `FrameAction` or `LogAction`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Loop<A> {
    /// Value for the counter to start at.
    pub(crate) start: NumConstant<usize>,
    /// Value for the counter to end at (inclusive).
    pub(crate) end: NumConstant<usize>,
    /// Sequence of Actions to loop.
    pub(crate) schedule: Vec<A>,
}

/// Set the timestamp metadata.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Timestamp {
    /// Sets timestamp to current datetime.
    Now,
    /// Sets timestamp to specific datetime.
    To(DateTime<Utc>),
    /// Increases current value of the timestamp by the given number of milliseconds.
    AdvanceByMs(usize),
    /// Decreases current value of the timestamp by the given number of milliseconds.
    RewindByMs(usize),
}

/// Specifies the level an emitted trace should be at when a `TracingEvent` action is called.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TracingLevel {
    /// Emit the tracing event at `Info` level.
    Info,
    /// Emit the tracing event at `debug` level.
    Debug,
}

/// Specifies the tracing event to be emitted.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct TracingEvent {
    /// The level of the tracing event.
    pub(crate) level: TracingLevel,
    /// The text of the tracing event.
    pub(crate) message: String,
}

/// A toplevel action.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Action {
    /// This action does nothing, and is used to annotate the script.
    Comment(#[allow(unused)] String),
    /// Emit a tracing event.
    TracingEvent(TracingEvent),
    /// Pauses the simulation for the given number of milliseconds.
    WaitMs(usize),
    /// Checks whether the given number of milliseconds has passed since ???, and pauses if not.
    EnsureDelayMs(usize),
    /// Send a runstart message.
    SendRunStart(SendRunStart),
    /// Send a runstop message.
    SendRunStop(SendRunStop),
    /// Send a runlog message.
    SendRunLogData(SendRunLogData),
    /// Send a selog message.
    SendSampleEnvLog(SendSampleEnvLog),
    /// Send an alarm message.
    SendAlarm(SendAlarm),
    /// Loop the given frame actions over the the given frame bounds.
    /// 
    /// This action sets the framenumber metadata located in [SimulationEngineState::metadata].
    FrameLoop(Loop<FrameAction>),
    /// Loop the given log actions over the the given bounds.
    LogLoop(Loop<LogAction>),
    /// Set the metadata timestamp field.
    SetTimestamp(Timestamp),
    /// Set the metadata vetoflags.
    SetVetoFlags(NumConstant<u16>),
    /// Set the metadata period field.
    SetPeriod(NumConstant<u64>),
    /// Set the metadata protons per pulse field.
    SetProtonsPerPulse(NumConstant<u8>),
    /// Set the metadata running flag.
    SetRunning(bool),
    /// Generate a trace, and place it in the cache.
    GenerateTrace(GenerateTrace),
    /// Generate an eventlist, and place it in the cache.
    GenerateEventList(GenerateEventList),
}

/// Actions that run in a frame loop context.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FrameAction {
    /// This action does nothing, and is used to annotate the script.
    Comment(#[allow(unused)] String),
    /// Pauses the simulation for the given number of milliseconds.
    WaitMs(usize),
    /// Checks whether the given number of milliseconds has passed since ???, and pauses if not.
    EnsureDelayMs(usize),
    /// Emit a tracing event.
    TracingEvent(TracingEvent),
    /// Sends a frame eventlist message.
    SendAggregatedFrameEventList(SendAggregatedEventListOptions),
    /// Loop the given digitiser actions over the the given digitiser index bounds.
    /// 
    /// This action sets the field [SimulationEngineState::digitiser_index], these indices
    /// correspond to digitiser ids in [DigitiserConfig].
    DigitiserLoop(Loop<DigitiserAction>),
    /// Set the metadata timestamp field.
    SetTimestamp(Timestamp),
    /// Generate a trace, and place it in the cache.
    GenerateTrace(GenerateTrace),
    /// Generate an eventlist, and place it in the cache.
    GenerateEventList(GenerateEventList),
}

/// Actions that run in a digitiser loop context.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DigitiserAction {
    /// This action does nothing, and is used to annotate the script.
    Comment(#[allow(unused)] String),
    /// Pauses the simulation for the given number of milliseconds.
    WaitMs(usize),
    /// Checks whether the given number of milliseconds has passed since ???, and pauses if not.
    EnsureDelayMs(usize),
    /// Emit a tracing event.
    TracingEvent(TracingEvent),
    /// Sends a digitiser trace message.
    SendDigitiserTrace(SendTraceOptions),
    /// Sends a digitiser eventlist message.
    SendDigitiserEventList(SendDigitiserEventListOptions),
    /// Generate a trace, and place it in the cache.
    GenerateTrace(GenerateTrace),
    /// Generate an eventlist, and place it in the cache.
    GenerateEventList(GenerateEventList),
    /// Generate an eventlist and corresponding trace, and place them in their respective caches.
    GenerateEventListAndTraces(GenerateEventList),
}

/// Actions that run in a log loop context.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum LogAction {
    /// This action does nothing, and is used to annotate the script.
    Comment(#[allow(unused)] String),
    /// Send a runlog message.
    SendRunLogData(SendRunLogData),
    /// Send a selog message.
    SendSampleEnvLog(SendSampleEnvLog),
    /// Send an alarm message.
    SendAlarm(SendAlarm),
    /// Set the metadata timestamp field.
    SetTimestamp(Timestamp),
    /// Pauses the simulation for the given number of milliseconds.
    WaitMs(usize),
}
