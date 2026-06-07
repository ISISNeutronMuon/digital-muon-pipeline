//! # Events Analyser
//!
//! The Events Analyser performs the following functions:
//! * Subscribes to a Kafka broker and to topics specified by the user.
//! * Runs persistantly, and awaits broker messages issued by the Event Formation units.
//! * Responds to messages sent via the broker which create new frames event lists, and appends real-time digitiser event lists them.
//! * Dispatches frame event lists when they are complete, or live past a user specified expiration time.
//! * Emits OpenTelemetry traces, integrated with traces from the rest of the pipeline.
//!
//! ## Features
//! * Employs multithreading to allow messages to be dispatched whilst waiting for digitiser messages.
//! * Records completion status of a frame event list message as well as all digitiser ids that contributed to it.
//! * Ignores any digitiser message whose timestamp is before the that of last frame event list to be dispatched.
//! * Ignores any digitiser message whose [id] and [metadata] have already been seen.
//!
//! ## Assumptions
//! * That each [DigitizerEventListMessage] has equally sized event fields (i.e. [time], [channel], and [voltage] are
//!   present and are all of equal length). This is guaranteed by the `trace-to-events` component.
//! * That the time stamps of [DigitizerEventListMessage] are correct.
//!
//! ## Error Conditions
//! * Missing fields of the [DigitizerEventListMessage] will cause it to be ignored.
//! * If a single digitser message has metadata timestamp set to a future time,
//!   this will cause the component to reject all subsequent messages (correctly timestamped)
//!   until the time of the erroneous future timestamp arrives.
//! * If a digitser message has metadata timestamp set earlier than intended, it will be ignored
//!   unless it happens to be before the timestamp of the last message to be dispatched.
//!   In this case the digitser message may be inserted into the wrong frame, or may result in a
//!   new (erroneous) frame.
//!
//! [time]: DigitizerEventListMessage::time
//! [channel]: DigitizerEventListMessage::channel
//! [voltage]: DigitizerEventListMessage::voltage
//! [id]: DigitizerEventListMessage::digitizer_id()
//! [metadata]: DigitizerEventListMessage::metadata()
mod analysis;
mod engine;
mod evaluator_task;
mod event;
mod eventlists;

use clap::{Parser, Subcommand};
use digital_muon_common::{
    CommonKafkaOpts, init_tracer,
    metrics::{
        component_info_metric,
        failures::{self, FailureKind},
        messages_received::{self, MessageKind},
        names::{FAILURES, FRAMES_SENT, MESSAGES_PROCESSED, MESSAGES_RECEIVED},
    },
    record_metadata_fields_to_span,
    tracer::{OptionalHeaderTracerExt, TracerEngine, TracerOptions},
};
use digital_muon_streaming_types::{
    dev2_digitizer_event_v2_generated::{
        DigitizerEventListMessage, digitizer_event_list_message_buffer_has_identifier,
        root_as_digitizer_event_list_message,
    },
    flatbuffers::InvalidFlatbuffer,
};
use eventlists::{EventlistsCollection, MessageCache};
use metrics::counter;
use metrics_exporter_prometheus::PrometheusBuilder;
use miette::{Context, IntoDiagnostic};
use rdkafka::{
    consumer::{CommitMode, Consumer},
    message::{BorrowedMessage, Message},
};
use std::{fs::File, net::SocketAddr, path::PathBuf, time::Duration};
use tokio::{
    signal::unix::{SignalKind, signal},
    sync::mpsc::{Sender, error::SendError},
};
use tracing::{debug, error, info_span, instrument, warn};

use crate::{
    analysis::{AnalysisEngine, ChartOutput},
    engine::AnalysisSettings,
    evaluator_task::create_evaluator_task,
};

// /// Triggers error if the producer takes longer than this to dispatch a message.
//const PRODUCER_TIMEOUT: Timeout = Timeout::After(Duration::from_millis(100));

/// [clap] derived struct to handle command line parameters.
#[derive(Parser)]
#[clap(author, version = digital_muon_common::version!(), about)]
struct Cli {
    /// Endpoint on which Prometheus text format metrics are available
    #[clap(long, env, default_value = "127.0.0.1:9090")]
    observability_address: SocketAddr,

    /// If set, then OpenTelemetry data is sent to the URL specified, otherwise the standard tracing subscriber is used
    #[clap(long)]
    otel_endpoint: Option<String>,

    /// All OpenTelemetry spans are emitted with this as the "service.namespace" property. Can be used to track different instances of the pipeline running in parallel.
    #[clap(long, default_value = "")]
    otel_namespace: String,

    /// Kafka consumer group
    #[clap(long)]
    chart_output: PathBuf,

    #[command(subcommand)]
    mode: Mode,
}

#[derive(Clone, Subcommand)]
enum Mode {
    Evaluate(Evaluation),
    ConvertJson(ConvertJson),
}

#[derive(Clone, Parser)]
struct Evaluation {
    #[clap(flatten)]
    common_kafka_options: CommonKafkaOpts,

    /// Kafka consumer group
    #[clap(long = "group")]
    consumer_group: String,

    /// Frame TTL in milliseconds.
    /// The time in which messages for a given eventlist must have been received from all topics.
    #[clap(long, default_value = "500")]
    eventlist_ttl_ms: u64,

    /// Frame cache poll interval in milliseconds.
    /// This may affect the rate at which incomplete frames are transmitted.
    #[clap(long, default_value = "500")]
    cache_poll_ms: u64,

    /// Size of the send frame buffer.
    /// If this limit is exceeded, the component will exit.
    #[clap(long, default_value = "1024")]
    send_frame_buffer_size: usize,

    /// Path to JSON file containing .
    #[clap(long)]
    analysis_settings: PathBuf,

    /// Flag to determine whether to load metric data automatically.
    #[clap(long)]
    load_metrics: bool,
}

#[derive(Clone, Parser)]
struct ConvertJson {
    file_name: String,
}

/// Entry point.
#[tokio::main]
async fn main() -> miette::Result<()> {
    let args = Cli::parse();

    let tracer = init_tracer!(TracerOptions::new(
        args.otel_endpoint.as_deref(),
        args.otel_namespace
    ));

    let eval_args = match args.mode {
        Mode::Evaluate(eval_args) => eval_args,
        Mode::ConvertJson(conv_json_args) => {
            ChartOutput::load_json(&args.chart_output, &conv_json_args.file_name)
                .into_diagnostic()?
                .save_plotly(&args.chart_output)
                .into_diagnostic()?;
            return Ok(());
        }
    };

    let kafka_opts = eval_args.common_kafka_options;

    let analysis_settings: AnalysisSettings =
        serde_json::from_reader(File::open(&eval_args.analysis_settings).into_diagnostic()?)
            .into_diagnostic()?;

    let topics = analysis_settings.events_topics.clone();
    let consumer = digital_muon_common::create_default_consumer(
        &kafka_opts.broker,
        &kafka_opts.username,
        &kafka_opts.password,
        &eval_args.consumer_group,
        Some(
            topics
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
    )
    .into_diagnostic()?;

    let ttl = Duration::from_millis(eval_args.eventlist_ttl_ms);

    let mut cache = MessageCache::new(ttl, analysis_settings.events_topics.len());

    // Install exporter and register metrics
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(args.observability_address)
        .install()
        .expect("Prometheus metrics exporter should be setup");

    metrics::describe_counter!(
        MESSAGES_RECEIVED,
        metrics::Unit::Count,
        "Number of messages received"
    );
    metrics::describe_counter!(
        MESSAGES_PROCESSED,
        metrics::Unit::Count,
        "Number of messages processed"
    );
    metrics::describe_counter!(
        FAILURES,
        metrics::Unit::Count,
        "Number of failures encountered"
    );
    metrics::describe_counter!(
        FRAMES_SENT,
        metrics::Unit::Count,
        "Number of complete frames sent by the aggregator"
    );

    let mut cache_poll_interval =
        tokio::time::interval(Duration::from_millis(eval_args.cache_poll_ms));

    let analysis_engine =
        AnalysisEngine::new(analysis_settings, args.chart_output, eval_args.load_metrics)
            .expect("FIXME: This may fail.");

    // Creates Send-Frame thread and returns channel sender
    let (channel_send, evaluator_task_handle) = create_evaluator_task(
        tracer.use_otel(),
        analysis_engine,
        eval_args.send_frame_buffer_size,
    )
    .into_diagnostic()?;

    // Is used to await any sigint signals
    let mut sigint = signal(SignalKind::interrupt()).into_diagnostic()?;

    component_info_metric("digitiser-aggregator");

    while !evaluator_task_handle.is_finished() {
        tokio::select! {
            event = consumer.recv() => {
                match event {
                    Ok(msg) => {
                        let span = info_span!("message_received");
                        msg.headers().conditional_extract_to_span(tracer.use_otel(), &span);
                        let _guard = span.enter();
                        process_kafka_message(&channel_send, &mut cache, topics.as_slice(), &msg)
                            .await
                            .into_diagnostic()
                            .wrap_err("Failed to process incomming message")?;

                        consumer.commit_message(&msg, CommitMode::Async)
                            .expect("Message should commit");
                    }
                    Err(e) => warn!("Kafka error: {}", e),
                };
            }
            _ = cache_poll_interval.tick() => {
                cache_poll(&channel_send, &mut cache).await.into_diagnostic()?;
            }
            _ = sigint.recv() => {
                //  Wait for the channel to close and
                //  all pending production tasks to finish
                evaluator_task_handle.await.into_diagnostic()?;
                return Ok(());
            }
        }
    }
    Ok(())
}

///  This function wraps the [root_as_digitizer_event_list_message] function, allowing it to be instrumented.
#[instrument(skip_all, level = "trace", err(level = "warn"))]
fn spanned_root_as_digitizer_event_list_message(
    payload: &[u8],
) -> Result<DigitizerEventListMessage<'_>, InvalidFlatbuffer> {
    root_as_digitizer_event_list_message(payload)
}

/// Extracts the payload of a Kafka message and passes it to [process_digitiser_event_list_message]
/// # Parameters
/// - channel_send: send channel which takes [EventlistsCollection] objects to dispatch.
/// - cache: the cache in which frames are stored whilst awaiting digitiser messages.
/// - topics: the names of the topics the consumer is subscribed to.
/// - msg: the message.
///
/// [Span]: tracing::Span
#[instrument(skip_all, level = "info", err(level = "warn"))]
async fn process_kafka_message(
    channel_send: &Sender<EventlistsCollection>,
    cache: &mut MessageCache,
    topics: &[String],
    msg: &BorrowedMessage<'_>,
) -> Result<(), SendError<EventlistsCollection>> {
    if let Some(payload) = msg.payload() {
        if digitizer_event_list_message_buffer_has_identifier(payload) {
            counter!(
                MESSAGES_RECEIVED,
                &[messages_received::get_label(MessageKind::Event)]
            )
            .increment(1);
            match spanned_root_as_digitizer_event_list_message(payload) {
                Ok(data) => {
                    let kafka_timestamp_ms = msg.timestamp().to_millis().unwrap_or(-1);
                    let topic_index = topics
                        .iter()
                        .enumerate()
                        .find_map(|(index, topic)| (*topic == msg.topic()).then_some(index))
                        .unwrap(); // FIXME: Handle error
                    process_digitiser_event_list_message(
                        channel_send,
                        cache,
                        kafka_timestamp_ms,
                        topic_index,
                        data,
                    )
                    .await?;
                }
                Err(e) => {
                    warn!("Failed to parse message: {}", e);
                    counter!(
                        FAILURES,
                        &[failures::get_label(FailureKind::UnableToDecodeMessage)]
                    )
                    .increment(1);
                }
            }
        } else {
            warn!("Unexpected message type on topic \"{}\"", msg.topic());
            debug!("Message: {msg:?}");
            debug!("Payload size: {}", payload.len());
            counter!(
                MESSAGES_RECEIVED,
                &[messages_received::get_label(MessageKind::Unexpected)]
            )
            .increment(1);
        }
    }
    Ok(())
}

/// Processes a [DigitizerEventListMessage], pushing it to the given [MessageCache].
/// # Parameters
/// - channel_send: send channel which takes [AggregatedFrame] objects to dispatch.
/// - kafka_message_timestamp_ms: the timestamp in milliseconds as reported in the Kafka message header. Only used for tracing.
/// - cache: the cache in which frames are stored whilst awaiting digitiser messages.
/// - message: the digitiser message.
#[tracing::instrument(skip_all, fields(
    digitiser_id = message.digitizer_id(),
    kafka_message_timestamp_ms=kafka_message_timestamp_ms,
    metadata_timestamp,
    metadata_frame_number,
    metadata_period_number,
    metadata_veto_flags,
    metadata_protons_per_pulse,
    metadata_running,
    topic_index=topic_index,
    num_cached_frames = cache.get_num_partial_frames(),
))]
async fn process_digitiser_event_list_message(
    channel_send: &Sender<EventlistsCollection>,
    cache: &mut MessageCache,
    kafka_message_timestamp_ms: i64,
    topic_index: usize,
    message: DigitizerEventListMessage<'_>,
) -> Result<(), SendError<EventlistsCollection>> {
    match message.metadata().try_into() {
        Ok(metadata) => {
            // Push the current digitiser message to the frame cache, possibly creating a new partial frame
            if let Err(err) = cache.push(
                message.digitizer_id(),
                &metadata,
                topic_index,
                message.into(),
            ) {
                tracing::Span::current().record(err.into(), true);
            }

            record_metadata_fields_to_span!(&metadata, tracing::Span::current());
            debug!("Event packet: metadata: {:?}", message.metadata());

            cache_poll(channel_send, cache).await?;
        }
        Err(e) => {
            warn!("Invalid Metadata: {e}");
            counter!(
                FAILURES,
                &[failures::get_label(FailureKind::InvalidMetadata)]
            )
            .increment(1);
        }
    }
    Ok(())
}

/// Polls the given [MessageCache] to see if there are any [EventlistsCollection]s ready to be dispatched.
///
/// If there are, this function removes them from the cache and sends them to the given send channel.
/// # Parameters
/// - channel_send: send channel which takes [EventlistsCollection] objects to dispatch.
/// - cache: the cache in which [EventlistsCollection] are stored whilst awaiting their counterparts from other topics.
#[tracing::instrument(skip_all, level = "trace")]
async fn cache_poll(
    channel_send: &Sender<EventlistsCollection>,
    cache: &mut MessageCache,
) -> Result<(), SendError<EventlistsCollection>> {
    while let Some(eventlists_collection) = cache.poll() {
        // Reserves space in the message queue if it is available
        // Or waits for space if none is available.
        match channel_send.reserve().await {
            Ok(permit) => permit.send(eventlists_collection),
            Err(_) => {
                error!("Send-Frame Error");
                return Err(SendError(eventlists_collection));
            }
        }
    }
    Ok(())
}
