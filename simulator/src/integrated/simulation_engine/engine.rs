use crate::integrated::{
    Topics,
    send_messages::{
        SendError, send_aggregated_frame_event_list_message, send_alarm_command,
        send_digitiser_event_list_message, send_digitiser_trace_message, send_run_log_command,
        send_run_start_command, send_run_stop_command, send_se_log_command,
    },
    simulation::{Simulation, SimulationError},
    simulation_elements::{
        event_list::{EventList, Trace},
        utils::JsonValueError,
    },
    simulation_engine::actions::{
        Action, DigitiserAction, FrameAction, GenerateEventList, GenerateTrace, LogAction,
        Timestamp, TracingEvent, TracingLevel,
    },
};
use chrono::{DateTime, TimeDelta, Utc};
use digital_muon_common::{Channel, DigitizerId, FrameNumber};
use digital_muon_streaming_types::FrameMetadata;
use rdkafka::producer::FutureProducer;
use std::{collections::VecDeque, thread::sleep, time::Duration};
use thiserror::Error;
use tokio::task::JoinSet;
use tracing::{debug, info, instrument};

/// Stores the state of the simulation.
#[derive(Clone, Debug)]
pub(crate) struct SimulationEngineState {
    /// The metadata to be used in any sent messages.
    pub(super) metadata: FrameMetadata,
    /// The current digitiser index. These refer to digitiser ids in [DigitiserConfig].
    pub(super) digitiser_index: usize,
    /// Measures the delay when the `EnsureDelayMs` action is called.
    pub(super) delay_from: DateTime<Utc>,
}

impl Default for SimulationEngineState {
    fn default() -> Self {
        Self {
            metadata: FrameMetadata {
                timestamp: Utc::now(),
                period_number: 0,
                protons_per_pulse: 0,
                running: true,
                frame_number: 0,
                veto_flags: 0,
            },
            digitiser_index: Default::default(),
            delay_from: Utc::now(),
        }
    }
}

/// Configuration for a digitiser, derived from [DigitiserConfig].
pub(crate) struct SimulationEngineDigitiser {
    /// The id of the digitiser.
    pub(crate) id: DigitizerId,
    /// The indices of the digitiser's channels. These index into [SimulationEngine::channels].
    pub(crate) channel_indices: Vec<usize>,
}

impl SimulationEngineDigitiser {
    #[instrument(skip_all, name = "digitiser", fields(digitiser_id = id))]
    pub(crate) fn new(id: DigitizerId, channel_indices: Vec<usize>) -> Self {
        SimulationEngineDigitiser {
            id,
            channel_indices,
        }
    }
}

/// Contains references and settings from outside the object.
pub(crate) struct SimulationEngineExternals<'a> {
    pub(crate) use_otel: bool,
    pub(crate) producer: &'a FutureProducer,
    pub(crate) kafka_producer_thread_set: &'a mut JoinSet<()>,
    pub(crate) topics: Topics<'a>,
}

#[derive(Debug, Error)]
pub(crate) enum SimulationEngineError {
    #[error("Simulation Error: {0}")]
    Simulation(#[from] SimulationError),
    #[error("Send Error: {0}")]
    Send(#[from] SendError),
    #[error("Aggregated Frame Event List Channel Index {0} out of Range: {1}")]
    AggregatedFrameEventListChannelIndexOutOfRange(usize, usize),
    #[error("Json Numerical Error: {0}")]
    JsonNum(#[from] JsonValueError),
    #[error("checked_add_signed failed: {0}")]
    TimestampAdd(usize),
    #[error("checked_sub_signed failed: {0}")]
    TimestampSub(usize),
}

pub(crate) struct SimulationEngine<'a> {
    /// Contains state and settings from outside the object.
    externals: SimulationEngineExternals<'a>,
    /// Persistant state throughout the simulation.
    state: SimulationEngineState,
    /// Contains traces that can be used by `SendDigitiserTrace` actions.
    trace_cache: VecDeque<Trace>,
    /// Contains traces that can be used by `SendDigitiserEventList` and `SendAggregatedFrameEventList` actions.
    event_list_cache: VecDeque<EventList<'a>>,
    /// The simulation, derived from a json file.
    simulation: &'a Simulation,
    /// The unique ids of each channel in the simulation.
    channels: Vec<Channel>,
    /// The digitiser configuration of each digitiser in the simulation.
    digitiser_ids: Vec<SimulationEngineDigitiser>,
}

impl<'a> SimulationEngine<'a> {
    /// Creates new engine.
    ///
    /// # Parameters
    /// - externals: state and settings from the caller.
    /// - simulation: simulation, derived from a json file.
    pub(crate) fn new(
        externals: SimulationEngineExternals<'a>,
        simulation: &'a Simulation,
    ) -> Result<Self, SimulationEngineError> {
        Ok(Self {
            externals,
            simulation,
            state: Default::default(),
            trace_cache: Default::default(),
            event_list_cache: Default::default(),
            digitiser_ids: simulation.digitiser_config.generate_digitisers()?,
            channels: simulation.digitiser_config.generate_channels()?,
        })
    }
}

/// Sets the timestamp of the metadata.
///
/// # Parameters
/// - engine: the engine to use.
/// - timestamp: the timestamp value to use.
#[instrument(skip_all, level = "debug", err(level = "error"))]
fn set_timestamp(
    engine: &mut SimulationEngine,
    timestamp: &Timestamp,
) -> Result<(), SimulationEngineError> {
    match timestamp {
        Timestamp::Now => engine.state.metadata.timestamp = Utc::now(),
        Timestamp::To(ts) => engine.state.metadata.timestamp = *ts,
        Timestamp::AdvanceByMs(ms) => {
            engine.state.metadata.timestamp = engine
                .state
                .metadata
                .timestamp
                .checked_add_signed(TimeDelta::milliseconds(*ms as i64))
                .ok_or(SimulationEngineError::TimestampAdd(*ms))?
        }
        Timestamp::RewindByMs(ms) => {
            engine.state.metadata.timestamp = engine
                .state
                .metadata
                .timestamp
                .checked_sub_signed(TimeDelta::milliseconds(*ms as i64))
                .ok_or(SimulationEngineError::TimestampSub(*ms))?
        }
    }
    Ok(())
}

/// Waits for the given time.
///
/// # Parameters
/// - ms: time to pause in milliseconds.
#[instrument(skip_all, level = "debug")]
fn wait_ms(ms: usize) {
    sleep(Duration::from_millis(ms as u64));
}

/// Waits for the given time, if that duration has not already ellapsed.
///
/// # Parameters
/// - ms: time to pause in milliseconds.
/// - delay_from: time to measure the pause from.
#[instrument(skip_all, level = "debug")]
fn ensure_delay_ms(ms: usize, delay_from: &mut DateTime<Utc>) {
    let duration = TimeDelta::milliseconds(ms as i64);
    if Utc::now() - *delay_from < duration {
        sleep(Duration::from_millis(duration.num_milliseconds() as u64));
    }
    *delay_from = Utc::now();
}

/// Generate traces and push them to the cache.
///
/// # Parameters
/// - engine: the engine to use.
/// - generate_trace: the details of the traces.
#[instrument(skip_all, level = "debug", err(level = "error"))]
fn generate_trace_push_to_cache(
    engine: &mut SimulationEngine,
    generate_trace: &GenerateTrace,
) -> Result<(), SimulationEngineError> {
    let event_lists = engine.simulation.generate_event_lists(
        generate_trace.event_list_index,
        engine.state.metadata.frame_number,
        generate_trace.repeat,
    )?;
    let traces = engine
        .simulation
        .generate_traces(event_lists.as_slice(), engine.state.metadata.frame_number)?;
    engine.trace_cache.extend(traces);
    Ok(())
}

/// Generate eventlists and push them to the cache.
///
/// # Parameters
/// - engine: the engine to use.
/// - generate_event: the details of the eventlists.
#[instrument(skip_all, level = "debug", err(level = "error"))]
fn generate_event_lists_push_to_cache(
    engine: &mut SimulationEngine,
    generate_event: &GenerateEventList,
) -> Result<(), SimulationError> {
    let event_lists = engine.simulation.generate_event_lists(
        generate_event.event_list_index,
        engine.state.metadata.frame_number,
        generate_event.repeat,
    )?;
    engine.event_list_cache.extend(event_lists);
    Ok(())
}

/// Generate eventlists and corresponding traces and push them to their respective caches.
///
/// # Parameters
/// - engine: the engine to use.
/// - generate_event: the details of the eventlists.
#[instrument(skip_all, level = "debug", err(level = "error"))]
fn generate_event_lists_and_traces_push_to_cache(
    engine: &mut SimulationEngine,
    generate_event: &GenerateEventList,
) -> Result<(), SimulationError> {
    let event_lists = engine.simulation.generate_event_lists(
        generate_event.event_list_index,
        engine.state.metadata.frame_number,
        generate_event.repeat,
    )?;
    engine.event_list_cache.extend(event_lists.clone());
    let traces = engine
        .simulation
        .generate_traces(event_lists.as_slice(), engine.state.metadata.frame_number)?;
    engine.trace_cache.extend(traces);
    Ok(())
}

/// Emit a tracing event.
///
/// # Parameters
/// - event: the tracing event to emit.
#[instrument(skip_all, level = "debug")]
fn tracing_event(event: &TracingEvent) {
    match event.level {
        TracingLevel::Info => info!(event.message),
        TracingLevel::Debug => debug!(event.message),
    }
}

/// Runs through toplevel actions.
///
/// # Parameters
/// - engine: the engine to use.
#[tracing::instrument(skip_all, level = "debug", fields(num_actions = engine.simulation.schedule.len()), err(level = "error"))]
pub(crate) fn run_schedule(engine: &mut SimulationEngine) -> Result<(), SimulationEngineError> {
    for action in engine.simulation.schedule.iter() {
        match action {
            Action::WaitMs(ms) => wait_ms(*ms),
            Action::EnsureDelayMs(ms) => ensure_delay_ms(*ms, &mut engine.state.delay_from),
            Action::TracingEvent(event) => tracing_event(event),
            Action::SendRunStart(run_start) => send_run_start_command(
                &mut engine.externals,
                run_start,
                &engine.state.metadata.timestamp,
            )?,
            Action::SendRunStop(run_stop) => send_run_stop_command(
                &mut engine.externals,
                run_stop,
                &engine.state.metadata.timestamp,
            )?,
            Action::SendRunLogData(run_log_data) => send_run_log_command(
                &mut engine.externals,
                &engine.state.metadata.timestamp,
                run_log_data,
            )?,
            Action::SendSampleEnvLog(sample_env_log) => {
                send_se_log_command(
                    &mut engine.externals,
                    &engine.state.metadata.timestamp,
                    sample_env_log,
                )?;
            }
            Action::SendAlarm(alarm) => {
                send_alarm_command(
                    &mut engine.externals,
                    &engine.state.metadata.timestamp,
                    alarm,
                )?;
            }
            Action::SetVetoFlags(vetoes) => {
                engine.state.metadata.veto_flags = vetoes.value()?;
            }
            Action::SetPeriod(period) => {
                engine.state.metadata.period_number = period.value()?;
            }
            Action::SetProtonsPerPulse(ppp) => {
                engine.state.metadata.protons_per_pulse = ppp.value()?;
            }
            Action::SetRunning(running) => {
                engine.state.metadata.running = *running;
            }
            Action::GenerateTrace(generate_trace) => {
                generate_trace_push_to_cache(engine, generate_trace)?
            }
            Action::GenerateEventList(generate_event) => {
                generate_event_lists_push_to_cache(engine, generate_event)?
            }
            Action::SetTimestamp(timestamp) => set_timestamp(engine, timestamp)?,
            Action::FrameLoop(frame_loop) => {
                for frame in frame_loop.start.value()?..=frame_loop.end.value()? {
                    engine.state.metadata.frame_number = frame as FrameNumber;
                    run_frame(engine, frame_loop.schedule.as_slice())?;
                }
            }
            Action::LogLoop(log_loop) => {
                for index in log_loop.start.value()?..=log_loop.end.value()? {
                    engine.state.metadata.frame_number = index as FrameNumber;
                    run_logloop_schedule(engine, log_loop.schedule.as_slice())?;
                }
            }
            Action::Comment(_) => (),
        }
    }
    Ok(())
}

/// Runs through frame actions.
///
/// # Parameters
/// - engine: the engine to use.
/// - frame_actions: sequence of frame actions to run through.
#[tracing::instrument(
    skip_all, level = "debug"
    fields(
        frame_number = engine.state.metadata.frame_number,
        num_actions = frame_actions.len()
    ),
    err(level = "error")
)]
fn run_frame(
    engine: &mut SimulationEngine,
    frame_actions: &[FrameAction],
) -> Result<(), SimulationEngineError> {
    for action in frame_actions {
        match action {
            FrameAction::WaitMs(ms) => wait_ms(*ms),
            FrameAction::EnsureDelayMs(ms) => ensure_delay_ms(*ms, &mut engine.state.delay_from),
            FrameAction::TracingEvent(event) => tracing_event(event),
            FrameAction::SendAggregatedFrameEventList(source) => {
                send_aggregated_frame_event_list_message(
                    &mut engine.externals,
                    &mut engine.event_list_cache,
                    &engine.state.metadata,
                    &source
                        .channel_indices
                        .range_inclusive()
                        .map(|i| engine.channels
                            .get(i)
                            .copied()
                            .ok_or(SimulationEngineError::AggregatedFrameEventListChannelIndexOutOfRange(i, engine.channels.len()))
                        )
                        .collect::<Result<Vec<_>,SimulationEngineError>>()?,
                    &source.source_options,
                )?
            }
            FrameAction::GenerateTrace(generate_trace) => {
                generate_trace_push_to_cache(engine, generate_trace)?
            }
            FrameAction::GenerateEventList(generate_event) => {
                generate_event_lists_push_to_cache(engine, generate_event)?
            }
            FrameAction::SetTimestamp(timestamp) => set_timestamp(engine, timestamp)?,
            FrameAction::DigitiserLoop(digitiser_loop) => {
                for digitiser in digitiser_loop.start.value()?..=digitiser_loop.end.value()? {
                    engine.state.digitiser_index = digitiser;
                    run_digitiser(engine, &digitiser_loop.schedule)?;
                }
            }
            FrameAction::Comment(_) => (),
        }
    }
    Ok(())
}

/// Runs through digitiser actions.
///
/// # Parameters
/// - engine: the engine to use.
/// - digitiser_actions: sequence of digitiser actions to run through.
#[tracing::instrument(skip_all, level = "debug"
    fields(
        frame_number = engine.state.metadata.frame_number,
        digitiser_id = engine.digitiser_ids[engine.state.digitiser_index].id,
        num_actions = digitiser_actions.len()
    )
    err(level = "error")
)]
pub(crate) fn run_digitiser(
    engine: &mut SimulationEngine,
    digitiser_actions: &[DigitiserAction],
) -> Result<(), SimulationEngineError> {
    for action in digitiser_actions {
        match action {
            DigitiserAction::WaitMs(ms) => wait_ms(*ms),
            DigitiserAction::EnsureDelayMs(ms) => {
                ensure_delay_ms(*ms, &mut engine.state.delay_from)
            }
            DigitiserAction::TracingEvent(event) => tracing_event(event),
            DigitiserAction::SendDigitiserTrace(source) => {
                let digitiser = engine
                    .digitiser_ids
                    .get(engine.state.digitiser_index)
                    .ok_or(
                        SimulationEngineError::AggregatedFrameEventListChannelIndexOutOfRange(
                            engine.state.digitiser_index,
                            engine.digitiser_ids.len(),
                        ),
                    )?;
                send_digitiser_trace_message(
                    &mut engine.externals,
                    engine.simulation.sample_rate.value()?,
                    &mut engine.trace_cache,
                    &engine.state.metadata,
                    digitiser.id,
                    &digitiser
                        .channel_indices
                        .iter()
                        .map(|idx| engine.channels[*idx])
                        .collect::<Vec<_>>(),
                    source.0,
                )?;
            }
            DigitiserAction::SendDigitiserEventList(source) => {
                let digitiser = engine
                    .digitiser_ids
                    .get(engine.state.digitiser_index)
                    .ok_or(
                        SimulationEngineError::AggregatedFrameEventListChannelIndexOutOfRange(
                            engine.state.digitiser_index,
                            engine.digitiser_ids.len(),
                        ),
                    )?;
                send_digitiser_event_list_message(
                    &mut engine.externals,
                    &mut engine.event_list_cache,
                    &engine.state.metadata,
                    digitiser.id,
                    &digitiser
                        .channel_indices
                        .iter()
                        .map(|idx| engine.channels[*idx])
                        .collect::<Vec<_>>(),
                    &source.0,
                )?;
            }
            DigitiserAction::GenerateTrace(generate_trace) => {
                generate_trace_push_to_cache(engine, generate_trace)?
            }
            DigitiserAction::GenerateEventList(generate_event) => {
                generate_event_lists_push_to_cache(engine, generate_event)?
            }
            DigitiserAction::GenerateEventListAndTraces(generate_event) => {
                generate_event_lists_and_traces_push_to_cache(engine, generate_event)?
            }
            DigitiserAction::Comment(_) => (),
        }
    }
    Ok(())
}

/// Runs through log actions.
///
/// # Parameters
/// - engine: the engine to use.
/// - log_actions: sequence of log actions to run through.
#[tracing::instrument(skip_all, level = "debug"
    fields(
        index = engine.state.metadata.frame_number,
        num_actions = log_actions.len()
    )
    err(level = "error")
)]
pub(crate) fn run_logloop_schedule(
    engine: &mut SimulationEngine,
    log_actions: &[LogAction],
) -> Result<(), SimulationEngineError> {
    for action in log_actions {
        match action {
            LogAction::WaitMs(ms) => wait_ms(*ms),
            LogAction::SendRunLogData(run_log_data) => send_run_log_command(
                &mut engine.externals,
                &engine.state.metadata.timestamp,
                run_log_data,
            )?,
            LogAction::SendSampleEnvLog(sample_env_log) => send_se_log_command(
                &mut engine.externals,
                &engine.state.metadata.timestamp,
                sample_env_log,
            )?,
            LogAction::SendAlarm(alarm) => {
                send_alarm_command(
                    &mut engine.externals,
                    &engine.state.metadata.timestamp,
                    alarm,
                )?;
            }
            LogAction::SetTimestamp(timestamp) => set_timestamp(engine, timestamp)?,
            LogAction::Comment(_) => (),
        }
    }
    Ok(())
}
