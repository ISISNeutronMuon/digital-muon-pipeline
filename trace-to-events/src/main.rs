//! # Trace to Events
//!
//! The Trace to Events component performs the following functions:
//! * Subscribes to a Kafka broker and to a "trace" topic specified by the user.
//! * Runs persistantly, and awaits broker messages issued by the DAQs.
//! * Consumes digitisier trace messages, and applies the user specified event formation algorithm on it.
//! * For each trace message, produces a digitiser event list message to an "event list" topic, specified by the user.
//!
mod channels;
mod parameters;
mod processing;
mod pulse_detection;
#[cfg(test)]
mod test_data;

use crate::processing::DigitiserMessageProcessor;
use chrono::{DateTime, Utc};
use clap::Parser;
use const_format::concatcp;
use digital_muon_common::{
    CommonKafkaOpts, init_tracer,
    metrics::{
        component_info_metric,
        failures::{self, FailureKind},
        messages_received::{self, MessageKind},
        names::{
            FAILURES, LAST_MESSAGE_FRAME_NUMBER, LAST_MESSAGE_TIMESTAMP, MESSAGES_PROCESSED,
            MESSAGES_RECEIVED, METRIC_NAME_PREFIX,
        },
    },
    record_metadata_fields_to_span,
    tracer::{FutureRecordTracerExt, OptionalHeaderTracerExt, TracerEngine, TracerOptions},
};
use digital_muon_streaming_types::{
    FrameMetadata,
    dat2_digitizer_analog_trace_v2_generated::{
        DigitizerAnalogTraceMessage, digitizer_analog_trace_message_buffer_has_identifier,
        root_as_digitizer_analog_trace_message,
    },
    flatbuffers::{FlatBufferBuilder, InvalidFlatbuffer},
};
use metrics::{counter, describe_counter, describe_gauge, gauge};
use metrics_exporter_prometheus::PrometheusBuilder;
use miette::IntoDiagnostic;
use parameters::DetectorSettings;
use rdkafka::{
    Message, Offset, TopicPartitionList,
    consumer::{CommitMode, Consumer, StreamConsumer},
    message::BorrowedMessage,
    producer::{DeliveryFuture, FutureProducer, FutureRecord},
    util::Timeout,
};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    select,
    signal::unix::{Signal, SignalKind, signal},
    sync::mpsc::{Receiver, Sender, error::TrySendError},
    task::JoinHandle,
};
use tracing::{debug, error, info, info_span, instrument, trace, warn};

type InstrumentedDeliveryFuture = tracing::instrument::Instrumented<DeliveryFuture>;
type DigitiserEventListToBufferSender = Sender<InstrumentedDeliveryFuture>;
type TrySendDigitiserEventListError = TrySendError<InstrumentedDeliveryFuture>;

const EVENTS_FOUND_METRIC: &str = concatcp!(METRIC_NAME_PREFIX, "events_found");

/// Encapsulates parameters related to message producing used by
/// the `process_digitiser_trace_message` function.
struct SenderParameters<'a> {
    /// Topic to which eventlists should be produced.
    event_topic: &'a str,
    /// Send channel which takes [DeliveryFuture] objects to dispatch.
    sender: &'a DigitiserEventListToBufferSender,
    /// The Kafka producer which dispatches event lists to the broker.
    producer: &'a FutureProducer,
}

/// [clap] derived struct to handle command line parameters.
#[derive(Debug, Parser)]
#[clap(author, version = digital_muon_common::version!(), about)]
struct Cli {
    #[clap(flatten)]
    common_kafka_options: CommonKafkaOpts,

    /// Kafka consumer group
    #[clap(long)]
    consumer_group: String,

    /// The Kafka topic that trace messages are consumed from
    #[clap(long)]
    trace_topic: String,

    /// Topic to publish digitiser event messages to
    #[clap(long)]
    event_topic: String,

    /// Kafka topic from which event formation configuration is consumed
    #[clap(long)]
    config_topic: String,

    /// Size of the send eventlist buffer.
    /// If this limit is exceeded, the component will exit.
    #[clap(long, default_value = "1024")]
    send_eventlist_buffer_size: usize,

    /// Endpoint on which OpenMetrics flavour metrics are available
    #[clap(long, env, default_value = "127.0.0.1:9090")]
    observability_address: SocketAddr,

    /// If set, then OpenTelemetry data is sent to the URL specified, otherwise the standard tracing subscriber is used
    #[clap(long)]
    otel_endpoint: Option<String>,

    /// All OpenTelemetry spans are emitted with this as the "service.namespace" property. Can be used to track different instances of the pipeline running in parallel.
    #[clap(long, default_value = "")]
    otel_namespace: String,
}

/// Queries the config topic on boot, seeking the last/most recent message across all partitions,
/// and applies it if present. Assigns all partitions to `Offset::End` afterwards so subsequent
/// receives only catch newly produced messages.
#[instrument(skip_all)]
async fn fetch_latest_config(
    consumer: &StreamConsumer,
    config_topic: &str,
) -> miette::Result<Option<DetectorSettings>> {
    const METADATA_TIMEOUT: Duration = Duration::from_secs(5);
    const WATERMARK_TIMEOUT: Timeout = Timeout::After(Duration::from_secs(5));
    const RECV_TIMEOUT: Duration = Duration::from_secs(5);

    let metadata = match consumer.fetch_metadata(Some(config_topic), METADATA_TIMEOUT) {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to fetch metadata for config topic \"{config_topic}\": {e}");
            return Ok(None);
        }
    };

    let topic_metadata = match metadata.topics().iter().find(|t| t.name() == config_topic) {
        Some(t) => t,
        None => {
            warn!("Topic \"{config_topic}\" not found in broker metadata");
            return Ok(None);
        }
    };

    let partitions = topic_metadata.partitions();
    if partitions.is_empty() {
        warn!("No partitions found for topic \"{config_topic}\"");
        return Ok(None);
    }

    let mut candidate_partitions = Vec::new();
    for p in partitions {
        match consumer.fetch_watermarks(config_topic, p.id(), WATERMARK_TIMEOUT) {
            Ok((low, high)) => {
                debug!(
                    "Config topic \"{}\" partition {}: low={}, high={}",
                    config_topic,
                    p.id(),
                    low,
                    high
                );
                if high > low {
                    candidate_partitions.push((p.id(), high - 1));
                }
            }
            Err(e) => {
                warn!(
                    "Failed to fetch watermarks for config topic \"{}\" partition {}: {}",
                    config_topic,
                    p.id(),
                    e
                );
            }
        }
    }

    let mut latest_payload: Option<(i64, Vec<u8>)> = None;

    for (partition_id, offset) in candidate_partitions {
        consumer.unassign().into_diagnostic()?;
        let mut tpl = TopicPartitionList::with_capacity(1);
        tpl.add_partition_offset(config_topic, partition_id, Offset::Offset(offset))
            .into_diagnostic()?;
        consumer.assign(&tpl).into_diagnostic()?;

        match tokio::time::timeout(RECV_TIMEOUT, consumer.recv()).await {
            Ok(Ok(borrowed_msg)) => {
                if let Some(payload) = borrowed_msg.payload() {
                    let ts = borrowed_msg.timestamp().to_millis().unwrap_or(0);
                    match &latest_payload {
                        Some((best_ts, _)) if ts <= *best_ts => {}
                        _ => {
                            latest_payload = Some((ts, payload.to_vec()));
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("Error receiving message from config topic partition {partition_id}: {e}");
            }
            Err(_) => {
                warn!("Timeout receiving message from config topic partition {partition_id}");
            }
        }
    }

    consumer.unassign().into_diagnostic()?;
    let mut tpl = TopicPartitionList::with_capacity(partitions.len());
    for p in partitions {
        tpl.add_partition_offset(config_topic, p.id(), Offset::End)
            .into_diagnostic()?;
    }
    consumer.assign(&tpl).into_diagnostic()?;

    if let Some((_, payload)) = latest_payload {
        match serde_json::from_slice::<DetectorSettings>(&payload) {
            Ok(settings) => Ok(Some(settings)),
            Err(e) => {
                warn!("Failed to parse JSON config message on boot: {e}");
                Ok(None)
            }
        }
    } else {
        Ok(None)
    }
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    let args = Cli::parse();

    let tracer = init_tracer!(TracerOptions::new(
        args.otel_endpoint.as_deref(),
        args.otel_namespace.clone()
    ));

    let kafka_opts = &args.common_kafka_options;

    let client_config = digital_muon_common::generate_kafka_client_config(
        &kafka_opts.broker,
        &kafka_opts.username,
        &kafka_opts.password,
    );

    let producer: FutureProducer = client_config.create().into_diagnostic()?;

    let trace_consumer = digital_muon_common::create_default_consumer(
        &kafka_opts.broker,
        &kafka_opts.username,
        &kafka_opts.password,
        &args.consumer_group,
        Some(&[args.trace_topic.as_str()]),
    )
    .into_diagnostic()?;

    let config_group = format!("{}-config-{}", args.consumer_group, uuid::Uuid::new_v4());
    let config_consumer = digital_muon_common::create_default_consumer(
        &kafka_opts.broker,
        &kafka_opts.username,
        &kafka_opts.password,
        &config_group,
        None,
    )
    .into_diagnostic()?;

    let initial_config = fetch_latest_config(&config_consumer, &args.config_topic).await?;
    let mut message_processor = if let Some(settings) = initial_config {
        info!(
            "Applied initial event formation configuration: {:?}",
            settings
        );
        Some(DigitiserMessageProcessor::new(8, &settings))
    } else {
        info!(
            "No initial configuration found on topic \"{}\"; incoming trace messages will be discarded until configuration is received",
            args.config_topic
        );
        None
    };

    // Install exporter and register metrics
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(args.observability_address)
        .install()
        .into_diagnostic()?;

    describe_counter!(
        MESSAGES_RECEIVED,
        metrics::Unit::Count,
        "Number of messages received"
    );
    describe_gauge!(
        LAST_MESSAGE_TIMESTAMP,
        "GPS sourced timestamp of the last received message from each digitizer"
    );
    describe_gauge!(
        LAST_MESSAGE_FRAME_NUMBER,
        "Frame number of the last received message from each digitizer"
    );
    describe_counter!(
        MESSAGES_PROCESSED,
        metrics::Unit::Count,
        "Number of messages processed"
    );
    describe_counter!(
        FAILURES,
        metrics::Unit::Count,
        "Number of failures encountered"
    );
    describe_counter!(
        EVENTS_FOUND_METRIC,
        metrics::Unit::Count,
        "Number of events found per channel"
    );

    let (sender, producer_task_handle) =
        create_producer_task(args.send_eventlist_buffer_size).into_diagnostic()?;

    // Is used to await any sigint signals
    let mut sigint = signal(SignalKind::interrupt()).into_diagnostic()?;

    component_info_metric("trace-to-events");

    let sender_parameters = SenderParameters {
        event_topic: &args.event_topic,
        sender: &sender,
        producer: &producer,
    };
    loop {
        tokio::select! {
            config_msg = config_consumer.recv() => match config_msg {
                Ok(m) => {
                    if let Some(payload) = m.payload() {
                        match serde_json::from_slice::<DetectorSettings>(payload) {
                            Ok(new_settings) => {
                                info!("Received new event formation configuration: {:?}", new_settings);
                                if let Some(ref mut processor) = message_processor {
                                    processor.reconfigure(&new_settings);
                                } else {
                                    message_processor = Some(DigitiserMessageProcessor::new(8, &new_settings));
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse configuration JSON on topic \"{}\": {}", m.topic(), e);
                                counter!(
                                    FAILURES,
                                    &[failures::get_label(FailureKind::UnableToDecodeMessage)]
                                )
                                .increment(1);
                            }
                        }
                    }
                }
                Err(e) => warn!("Kafka error on config topic: {}", e)
            },
            trace_msg = trace_consumer.recv() => match trace_msg {
                Ok(m) => {
                    if let Some(ref mut processor) = message_processor {
                        let span = info_span!("message_received");
                        m.headers().conditional_extract_to_span(tracer.use_otel(), &span);
                        let _guard = span.enter();
                        process_kafka_message(
                            &tracer,
                            &sender_parameters,
                            processor,
                            &m,
                        ).into_diagnostic()?;

                        trace_consumer.commit_message(&m, CommitMode::Async).unwrap();
                    } else {
                        debug!("Discarding trace message: no event formation configuration applied yet");
                        trace_consumer.commit_message(&m, CommitMode::Async).unwrap();
                    }
                }
                Err(e) => warn!("Kafka error on trace topic: {}", e)
            },
            _ = sigint.recv() => {
                //  Wait for the channel to close and
                //  all pending production tasks to finish
                producer_task_handle.await.into_diagnostic()?;
                return Ok(());
            }
        }
    }
}

///  This function wraps the [root_as_digitizer_analog_trace_message] function, allowing it to be instrumented.
#[instrument(skip_all, level = "trace", err(level = "warn"))]
fn spanned_root_as_digitizer_analog_trace_message(
    payload: &[u8],
) -> Result<DigitizerAnalogTraceMessage<'_>, InvalidFlatbuffer> {
    root_as_digitizer_analog_trace_message(payload)
}

/// Extracts the payload of a Kafka message and passes it to [process_digitiser_trace_message]
/// # Parameters
/// - tracer: the tracer object, this is used to call the [TracerEngine::user_otel()] method, this could be replaced by a [bool].
/// - sender_parameters: parameters specific to message producing.
/// - message_processor: the processor's persistant state object.
/// - message: the message to process.
///
/// [Span]: tracing::Span
#[instrument(skip_all, level = "info", err(level = "warn"))]
fn process_kafka_message(
    tracer: &TracerEngine,
    sender_parameters: &SenderParameters,
    message_processor: &mut DigitiserMessageProcessor,
    message: &BorrowedMessage,
) -> Result<(), TrySendDigitiserEventListError> {
    debug!(
        "key: '{:?}', topic: {}, partition: {}, offset: {}, timestamp: {:?}",
        message.key(),
        message.topic(),
        message.partition(),
        message.offset(),
        message.timestamp()
    );

    if let Some(payload) = message.payload() {
        if digitizer_analog_trace_message_buffer_has_identifier(payload) {
            match spanned_root_as_digitizer_analog_trace_message(payload) {
                Ok(trace_message) => {
                    let kafka_timestamp_ms = message.timestamp().to_millis().unwrap_or(-1);
                    let kafka_key = message.key_view::<str>().and_then(Result::ok);
                    process_digitiser_trace_message(
                        tracer,
                        kafka_timestamp_ms,
                        sender_parameters,
                        message_processor,
                        kafka_key,
                        trace_message,
                    )?
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
            warn!("Unexpected message type on topic \"{}\"", message.topic());
            counter!(
                MESSAGES_RECEIVED,
                &[messages_received::get_label(MessageKind::Unexpected)]
            )
            .increment(1);
        }
    }
    Ok(())
}

/// Processes a [DigitizerAnalogTraceMessage].
/// # Parameters
/// - tracer: the tracer object, this is used to call the [TracerEngine::user_otel()] method, this could be replaced by a [bool].
/// - headers: the Kafka header of the message.
/// - args: the user-specified Cli arguments.
/// - sender: send channel which takes [DeliveryFuture] objects to dispatch.
/// - kafka_timestamp_ms: the timestamp in milliseconds as reported in the Kafka message header. Only used for tracing.
/// - message: the digitiser message.
#[instrument(
    skip_all,
    fields(
        digitiser_id = message.digitizer_id(),
        kafka_message_timestamp_ms = kafka_timestamp_ms,
        send_digitiser_eventlist_buffer_capcacity,
        metadata_timestamp,
        metadata_frame_number,
        metadata_period_number,
        metadata_veto_flags,
        metadata_protons_per_pulse,
        metadata_running,
        num_total_pulses,
    )
)]
fn process_digitiser_trace_message(
    tracer: &TracerEngine,
    kafka_timestamp_ms: i64,
    sender_parameters: &SenderParameters,
    message_processor: &mut DigitiserMessageProcessor,
    kafka_key: Option<&str>,
    message: DigitizerAnalogTraceMessage,
) -> Result<(), TrySendDigitiserEventListError> {
    let did = format!("{}", message.digitizer_id());

    counter!(
        MESSAGES_RECEIVED,
        &[
            messages_received::get_label(MessageKind::Trace),
            ("digitizer_id", did.clone())
        ]
    )
    .increment(1);

    let timestamp: Option<DateTime<Utc>> = message
        .metadata()
        .timestamp()
        .copied()
        .and_then(|v| v.try_into().ok());
    if let Some(timestamp) = timestamp {
        gauge!(
            LAST_MESSAGE_TIMESTAMP,
            &[
                messages_received::get_label(MessageKind::Trace),
                ("digitizer_id", did.clone())
            ]
        )
        // `timestamp_nanos_opt` returns `None` when the year is >2262. This is long after this
        // software will be of use.
        .set(timestamp.timestamp_nanos_opt().unwrap() as f64);
    } else {
        warn!(
            "Failed to update {LAST_MESSAGE_TIMESTAMP} metric due to malformed message/timestamp"
        );
    }

    gauge!(
        LAST_MESSAGE_FRAME_NUMBER,
        &[
            messages_received::get_label(MessageKind::Trace),
            ("digitizer_id", did)
        ]
    )
    .set(message.metadata().frame_number() as f64);

    message
        .metadata()
        .try_into()
        .inspect(|metadata: &FrameMetadata| {
            record_metadata_fields_to_span!(metadata, tracing::Span::current());
        })
        .ok();

    let mut fbb: FlatBufferBuilder<'_> = FlatBufferBuilder::new();
    let num_total_pulses = message_processor.process(&mut fbb, &message);
    tracing::Span::current().record("num_total_pulses", num_total_pulses);
    tracing::Span::current().record(
        "send_digitiser_eventlist_buffer_capcacity",
        sender_parameters.sender.capacity(),
    );

    let mut future_record = FutureRecord::to(sender_parameters.event_topic)
        .payload(fbb.finished_data())
        .conditional_inject_current_span_into_headers(tracer.use_otel());
    if let Some(kafka_key) = kafka_key {
        future_record = future_record.key(kafka_key);
    }

    let future = sender_parameters
        .producer
        .send_result(future_record)
        .expect("Producer sends");

    if let Err(e) = sender_parameters
        .sender
        .try_send(tracing::Instrument::instrument(
            future,
            tracing::Span::current(),
        ))
    {
        match &e {
            TrySendError::Closed(_) => {
                error!("Send-Frame Channel Closed");
            }
            TrySendError::Full(_) => {
                error!("Send-Frame Buffer Full");
            }
        }
        Err(e)
    } else {
        Ok(())
    }
}

// The following functions control the kafka producer thread.

/// Create a new thread and setup the producer task.
/// # Parameters
/// - send_digitiser_eventlist_buffer_size: the maximum number of [DeliveryFuture] objects to store in the channel's buffer. If the buffer is filled, then sending another frame will block until there is sufficient space in the buffer.
fn create_producer_task(
    send_digitiser_eventlist_buffer_size: usize,
) -> std::io::Result<(DigitiserEventListToBufferSender, JoinHandle<()>)> {
    let (channel_send, channel_recv) = tokio::sync::mpsc::channel::<InstrumentedDeliveryFuture>(
        send_digitiser_eventlist_buffer_size,
    );

    let sigint = signal(SignalKind::interrupt())?;
    let handle = tokio::spawn(produce_to_kafka(channel_recv, sigint));
    Ok((channel_send, handle))
}

/// Runs infinitely, and waits on any [DeliveryFuture]s received through the given receive channel.
///
/// Calling this function returns a Future, which should be passed to a async task,
/// as in function [create_producer_task]. The general form of this is:
/// ```rust
/// let join_handle = tokio::spawn(produce_to_kafka(...))?;
/// ```
/// # Parameters
/// - channel_recv: receive channel that can receive [DeliveryFuture] objects.
/// - sigint: triggers when the os sends a signal to the process.
async fn produce_to_kafka(
    mut channel_recv: Receiver<InstrumentedDeliveryFuture>,
    mut sigint: Signal,
) {
    loop {
        // Blocks until a frame is received
        select! {
            message = channel_recv.recv() => {
                match message {
                    Some(future) => {
                        produce_eventlist_to_kafka(future).await
                    },
                    None => {
                        info!("Send-Eventlist channel closed");
                        return;
                    }
                }
            },
            _ = sigint.recv() => {
                close_and_flush_producer_channel(&mut channel_recv).await;
            }
        }
    }
}

/// Dispatches the given eventlist to the Kafka broker by waiting the [DeliveryFuture].
/// # Parameters
/// - future: the future which produces the message.
#[instrument(skip_all, parent = future.span())]
async fn produce_eventlist_to_kafka(future: InstrumentedDeliveryFuture) {
    match future.await {
        Ok(_) => {
            trace!("Published event message");
            counter!(MESSAGES_PROCESSED).increment(1);
        }
        Err(e) => {
            error!("{:?}", e);
            counter!(
                FAILURES,
                &[failures::get_label(FailureKind::KafkaPublishFailed)]
            )
            .increment(1);
        }
    }
}

/// Closes the producer channel and dispatch all [DeliveryFuture]s remaining in the channel.
/// # Parameters
/// - channel_recv: receive channel that can receive [DeliveryFuture] objects.
#[tracing::instrument(skip_all, name = "Closing", level = "info", fields(capactity = channel_recv.capacity(), max_capactity = channel_recv.max_capacity()))]
async fn close_and_flush_producer_channel(
    channel_recv: &mut Receiver<InstrumentedDeliveryFuture>,
) -> Option<()> {
    channel_recv.close();

    loop {
        let future = channel_recv.recv().await?;
        flush_eventlist(future).await?;
    }
}

/// Dispatches the given future to the Kafka broker by calling and awaiting [produce_eventlist_to_kafka()].
///
/// This function exists just to encapsulate [produce_eventlist_to_kafka] in a span, it might be better to do this directly in [close_and_flush_producer_channel].
/// # Parameters
/// - future: the future to dispatch.
#[tracing::instrument(skip_all, name = "Flush Eventlist")]
async fn flush_eventlist(future: InstrumentedDeliveryFuture) -> Option<()> {
    produce_eventlist_to_kafka(future).await;
    Some(())
}
