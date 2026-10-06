pub(crate) mod active_pulses;
pub(crate) mod build_messages;
pub(crate) mod send_messages;
pub(crate) mod simulation;
pub(crate) mod simulation_elements;
pub(crate) mod simulation_engine;

use crate::Defined;
use rdkafka::producer::FutureProducer;
use simulation::{Simulation, SimulationError};
use simulation_engine::{
    SimulationEngine, SimulationEngineExternals, engine::SimulationEngineError, run_schedule,
};
use std::fs::File;
use thiserror::Error;
use tokio::task::JoinSet;
use tracing::{error, trace};

/// References to topics that various messages should produce to.
pub(crate) struct Topics<'a> {
    /// Topic where trace messages are produced to.
    pub(crate) traces: &'a str,
    /// Topic where digitiser eventlist messages are produced to.
    pub(crate) events: &'a str,
    /// Topic where frame eventlist messages are produced to.
    pub(crate) frame_events: &'a str,
    /// Topic where run start/stop messages are produced to.
    pub(crate) run_controls: &'a str,
    /// Topic where runlog messages are produced to.
    pub(crate) runlog: &'a str,
    /// Topic where sample environment messages are produced to.
    pub(crate) selog: &'a str,
    /// Topic where alarm messages are produced to.
    pub(crate) alarm: &'a str,
}

#[derive(Debug, Error)]
pub(crate) enum ConfiguredError {
    #[error("Simulation Error: {0}")]
    Simulation(#[from] SimulationError),
    #[error("Simulation Engine Error: {0}")]
    SimulationEngine(#[from] SimulationEngineError),
    #[error("Json Error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("File Error: {0}")]
    IO(#[from] std::io::Error),
}

/// Create and run a simulation specified by a [Defined] object.
/// 
/// # Parameters
/// - use_otel: should OTel be used for tracing.
/// - producer: the Kafka Producer to send the message.
/// - defined: the cli arguments to produce the simulation.
#[tracing::instrument(skip_all, err(level = "error"))]
pub(crate) async fn run_configured_simulation(
    use_otel: bool,
    producer: &FutureProducer,
    defined: Defined,
) -> Result<(), ConfiguredError> {
    let simulation: Simulation = serde_json::from_reader(File::open(defined.file)?)?;
    let mut kafka_producer_thread_set = JoinSet::<()>::new();
    let mut engine = SimulationEngine::new(
        SimulationEngineExternals {
            use_otel,
            producer,
            kafka_producer_thread_set: &mut kafka_producer_thread_set,
            topics: Topics {
                traces: &defined.digitiser_trace_topic,
                events: &defined.digitiser_event_topic,
                frame_events: &defined.frame_event_topic,
                run_controls: &defined.control_topic,
                runlog: &defined.runlog_topic,
                selog: &defined.selog_topic,
                alarm: &defined.alarm_topic,
            },
        },
        &simulation,
    )?;

    if let Err(e) = run_schedule(&mut engine) {
        error!("Critical Error: {e}");
    }

    trace!("Waiting for delivery threads to finish.");
    while let Some(result) = kafka_producer_thread_set.join_next().await {
        if let Err(e) = result {
            error!("{e}");
        }
    }

    trace!("All finished.");
    Ok(())
}
