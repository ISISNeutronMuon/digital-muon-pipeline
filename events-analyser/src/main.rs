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
mod event;
mod eventlists;

use clap::Parser;
use digital_muon_common::{
    CommonKafkaOpts, DigitizerId, init_tracer,
    metrics::{
        component_info_metric,
        failures::{self, FailureKind},
        messages_received::{self, MessageKind},
        names::{FAILURES, FRAMES_SENT, MESSAGES_PROCESSED, MESSAGES_RECEIVED},
    },
    record_metadata_fields_to_span,
    spanned::Spanned,
    tracer::{FutureRecordTracerExt, OptionalHeaderTracerExt, TracerEngine, TracerOptions},
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
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
};
use std::{fmt::Debug, net::SocketAddr, time::Duration};
use tokio::{
    select,
    signal::unix::{Signal, SignalKind, signal},
    sync::mpsc::{Receiver, Sender, error::SendError},
    task::JoinHandle,
};
use tracing::{debug, error, info, info_span, instrument, warn};

/// Triggers error if the producer takes longer than this to dispatch a message.
const PRODUCER_TIMEOUT: Timeout = Timeout::After(Duration::from_millis(100));

/// [clap] derived struct to handle command line parameters.
#[derive(Debug, Parser)]
#[clap(author, version = digital_muon_common::version!(), about)]
struct Cli {
    #[clap(flatten)]
    common_kafka_options: CommonKafkaOpts,

    /// Kafka consumer group
    #[clap(long = "group")]
    consumer_group: String,

    /// Kafka topic on which to emit frame assembled event messages
    /// Can be passed as `-etopic1 -etopic2 ...` or `-d=topic1,topic2,...`
    #[clap(short, long, value_delimiter = ',')]
    eventlist_topic: Vec<String>,

    /// A list of expected digitiser IDs.
    /// Can be passed as `-d0 -d1 ...` or `-d=0,1,...`
    /// A frame is only "complete" when a message has been received from each of these IDs.
    #[clap(short, long, value_delimiter = ',')]
    digitiser_ids: Vec<DigitizerId>,

    /// Frame TTL in milliseconds.
    /// The time in which messages for a given frame must have been received from all digitisers.
    #[clap(long, default_value = "500")]
    frame_ttl_ms: u64,

    /// Frame cache poll interval in milliseconds.
    /// This may affect the rate at which incomplete frames are transmitted.
    #[clap(long, default_value = "500")]
    cache_poll_ms: u64,

    /// Size of the send frame buffer.
    /// If this limit is exceeded, the component will exit.
    #[clap(long, default_value = "1024")]
    send_frame_buffer_size: usize,

    /// Endpoint on which Prometheus text format metrics are available
    #[clap(long, env, default_value = "127.0.0.1:9090")]
    observability_address: SocketAddr,

    /// If set, then OpenTelemetry data is sent to the URL specified, otherwise the standard tracing subscriber is used
    #[clap(long)]
    otel_endpoint: Option<String>,

    /// All OpenTelemetry spans are emitted with this as the "service.namespace" property. Can be used to track different instances of the pipeline running in parallel.
    #[clap(long, default_value = "")]
    otel_namespace: String,
}

/// Entry point.
#[tokio::main]
async fn main() -> miette::Result<()> {
    let args = Cli::parse();

    let tracer = init_tracer!(TracerOptions::new(
        args.otel_endpoint.as_deref(),
        args.otel_namespace
    ));

    let kafka_opts = args.common_kafka_options;

    let topics = args.eventlist_topic.iter().map(String::as_str).collect::<Vec<_>>();
    let consumer = digital_muon_common::create_default_consumer(
        &kafka_opts.broker,
        &kafka_opts.username,
        &kafka_opts.password,
        &args.consumer_group,
        Some(topics.as_slice()),
    )
    .into_diagnostic()?;

    let producer: FutureProducer = digital_muon_common::generate_kafka_client_config(
        &kafka_opts.broker,
        &kafka_opts.username,
        &kafka_opts.password,
    )
    .create()
    .into_diagnostic()?;

    let ttl = Duration::from_millis(args.frame_ttl_ms);

    let mut cache = MessageCache::new(ttl, args.eventlist_topic.len());

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

    let mut cache_poll_interval = tokio::time::interval(Duration::from_millis(args.cache_poll_ms));

    // Creates Send-Frame thread and returns channel sender
    let (channel_send, producer_task_handle) = create_evaluator_task(
        tracer.use_otel(),
        args.send_frame_buffer_size,
        //&producer,
        //&args.output_topic,
    )
    .into_diagnostic()?;

    // Is used to await any sigint signals
    let mut sigint = signal(SignalKind::interrupt()).into_diagnostic()?;

    component_info_metric("digitiser-aggregator");

    loop {
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
                producer_task_handle.await.into_diagnostic()?;
                return Ok(());
            }
        }
    }
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
/// - channel_send: send channel which takes [AggregatedFrame] objects to dispatch.
/// - cache: the cache in which frames are stored whilst awaiting digitiser messages.
/// - msg: the message.
///
/// [Span]: tracing::Span
#[instrument(skip_all, level = "info", err(level = "warn"))]
async fn process_kafka_message(
    channel_send: &Sender<EventlistsCollection>,
    cache: &mut MessageCache,
    topics: &[&str],
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
                    let topic_index = topics.iter().enumerate().find_map(|(index, topic)|(*topic == msg.topic()).then_some(index)).unwrap(); // FIXME: Handle error
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

/// Processes a [DigitizerEventListMessage], pushing it to the given [FrameCache].
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
    num_cached_frames = cache.get_num_partial_frames(),
    timestamp_too_early = false,
    id_already_present = false,
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
            debug!("Event packet: metadata: {:?}", message.metadata());

            // Push the current digitiser message to the frame cache, possibly creating a new partial frame
            if let Err(err) = cache.push(message.digitizer_id(), &metadata, topic_index, message.into()) {
                tracing::Span::current().record(err.into(), true);
            }

            record_metadata_fields_to_span!(&metadata, tracing::Span::current());

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

/// Polls the given [FrameCache] to see if there are any [AggregatedFrame]s ready to be dispatched.
///
/// If there are, this function removes them from the cache and sends them to the given send channel.
/// # Parameters
/// - channel_send: send channel which takes [AggregatedFrame] objects to dispatch.
/// - cache: the cache in which frames are stored whilst awaiting digitiser messages.
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

// The following functions control the kafka producer thread.
/// Create a new thread and setup the producer task.
/// # Parameters
/// - use_otel: if true, then the thread attempts to inject [AggregatedFrame::span()] into the Kafka header.
/// - send_frame_buffer_size: the maximum number of [AggregatedFrame] objects to store in the channel's buffer. If the buffer is filled, then sending another frame will block until there is sufficient space in the buffer.
/// - producer: the Kafka producer object.
/// - output_topic: the Kafka topic to produce the message to.
fn create_evaluator_task(
    use_otel: bool,
    send_frame_buffer_size: usize,
    //producer: &FutureProducer,
    //output_topic: &str,
) -> std::io::Result<(Sender<EventlistsCollection>, JoinHandle<()>)> {
    let (channel_send, channel_recv) =
        tokio::sync::mpsc::channel::<EventlistsCollection>(send_frame_buffer_size);

    let sigint = signal(SignalKind::interrupt())?;
    let handle = tokio::spawn(recv_and_evaluate(
        use_otel,
        channel_recv,
        //producer.to_owned(),
        //output_topic.to_owned(),
        sigint,
    ));
    Ok((channel_send, handle))
}

/// Runs infinitely, and waits on any [AggregatedFrame]s received through the given receive channel.
///
/// Calling this function returns a Future, which should be passed to a async task,
/// as in function [create_producer_task]. The general form of this is:
/// ```rust
/// let join_handle = tokio::spawn(produce_to_kafka(...))?;
/// ```
/// # Parameters
/// - use_otel: if true, then the thread attempts to inject [AggregatedFrame::span()] into the Kafka header.
/// - channel_recv: receive channel that can receive [AggregatedFrame] objects.
/// - producer: the Kafka producer object.
/// - output_topic: the Kafka topic to produce the message to.
async fn recv_and_evaluate(
    use_otel: bool,
    mut channel_recv: Receiver<EventlistsCollection>,
    //producer: FutureProducer,
    //output_topic: String,
    mut sigint: Signal,
) {
    loop {
        select! {
            message = channel_recv.recv() => {
                // Blocks until a frame is received
                match message {
                    Some(eventlists_collection) => {
                        evaluate_eventlists_collection(use_otel, eventlists_collection, /*&producer, &output_topic*/).await;
                    }
                    None => {
                        info!("Send-Frame channel closed");
                        return;
                    }
                }
            }
            _ = sigint.recv() => {
                close_and_flush_evaluate_channel(use_otel,&mut channel_recv, /*&producer,&output_topic*/).await;
            }
        }
    }
}

/// Closes the producer channel and dispatch all [AggregatedFrame]s remaining in the channel.
/// # Parameters
/// - use_otel: if true, then the thread attempts to inject [AggregatedFrame::span()] into the Kafka header.
/// - channel_recv: receive channel that can receive [AggregatedFrame] objects.
/// - producer: the Kafka producer object.
/// - output_topic: the Kafka topic to produce the message to.
#[tracing::instrument(skip_all, name = "Closing", level = "info", fields(capacity = channel_recv.capacity(), max_capacity = channel_recv.max_capacity()))]
async fn close_and_flush_evaluate_channel(
    use_otel: bool,
    channel_recv: &mut Receiver<EventlistsCollection>,
    //producer: &FutureProducer,
    //output_topic: &str,
) -> Option<()> {
    channel_recv.close();

    loop {
        let eventlists_collection = channel_recv.recv().await?;
        flush_eventlists_collection(use_otel, eventlists_collection,/*producer, output_topic*/).await?;
    }
}

/// Dispatches the given frame to the Kafka broker on the given topic.
///
/// This function exists just to encapsulate [produce_frame_to_kafka] in a span, it might be better to do this directly in [close_and_flush_producer_channel].
/// # Parameters
/// - use_otel: if true, then the thread attempts to inject [AggregatedFrame::span()] into the Kafka header.
/// - frame: the frame to dispatch.
/// - producer: the Kafka producer object.
/// - output_topic: the Kafka topic to produce the message to.|
#[tracing::instrument(skip_all, name = "Flush Frame")]
async fn flush_eventlists_collection(
    use_otel: bool,
    eventlists_collection: EventlistsCollection,
    //producer: &FutureProducer,
    //output_topic: &str,
) -> Option<()> {
    evaluate_eventlists_collection(use_otel, eventlists_collection, /*producer, output_topic*/).await;
    Some(())
}

/// Dispatches the given frame to the Kafka broker on the given topic.
/// # Parameters
/// - use_otel: if true, then the thread attempts to inject [AggregatedFrame::span()] into the Kafka header.
/// - frame: the frame to dispatch.
/// - producer: the Kafka producer object.
/// - output_topic: the Kafka topic to produce the message to.
#[tracing::instrument(skip_all)]
async fn evaluate_eventlists_collection(
    use_otel: bool,
    eventlists_collection: EventlistsCollection,
    //producer: &FutureProducer,
    //output_topic: &str,
) {
    println!("{eventlists_collection:?}");
    //let eventlists_collection_span = eventlists_collection.span().get().expect("Span should exist").clone();
    /*let data: Vec<u8> = frame.into();

    let future_record = FutureRecord::to(output_topic)
        .payload(data.as_slice())
        .conditional_inject_span_into_headers(use_otel, &frame_span)
        .key("Frame Events List");

    match producer.send(future_record, PRODUCER_TIMEOUT).await {
        Ok(r) => {
            debug!("Delivery: {:?}", r);
            counter!(FRAMES_SENT).increment(1)
        }
        Err(e) => {
            error!("Delivery failed: {:?}", e);
            counter!(
                FAILURES,
                &[failures::get_label(FailureKind::KafkaPublishFailed)]
            )
            .increment(1);
        }
    }*/
}
