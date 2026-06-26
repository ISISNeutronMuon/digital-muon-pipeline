//! # Trace Writer
//!
//! Subscribes to a Kafka topic carrying digitiser analog trace messages (`dat2`) and
//! writes the data to an HDF5 file.
//!
//! ## Modes
//!
//! **Continuous mode** (`--control-topic` is not set):
//! - A single HDF5 file is created at startup (path given by `--output`).
//! - Traces are appended until the process receives SIGINT.
//!
//! **Run-bounded mode** (`--control-topic` is set):
//! - `--output` is treated as a directory.
//! - A new HDF5 file is created for each `RunStart` message received on the
//!   control topic. The file is named `<run_name>.h5` inside `--output`.
//! - The current file is closed on every `RunStop` message.
//! - Trace messages that arrive when no file is open are warned about and
//!   discarded.

mod digitiser_data;
mod error;
mod file_writer;
mod trace_data;

use chrono::{DateTime, Utc};
use clap::Parser;
use digital_muon_common::{
    CommonKafkaOpts, init_tracer,
    metrics::{
        component_info_metric,
        failures::{self, FailureKind},
        messages_received::{self, MessageKind},
        names::{
            FAILURES, LAST_MESSAGE_FRAME_NUMBER, LAST_MESSAGE_TIMESTAMP, MESSAGES_PROCESSED,
            MESSAGES_RECEIVED,
        },
    },
    tracer::{OptionalHeaderTracerExt, TracerEngine, TracerOptions},
};
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::{
    digitizer_analog_trace_message_buffer_has_identifier, root_as_digitizer_analog_trace_message,
};
use file_writer::TraceFileWriter;
use isis_streaming_data_types::flatbuffers_generated::{
    run_start_pl72::{root_as_run_start, run_start_buffer_has_identifier},
    run_stop_6s4t::{root_as_run_stop, run_stop_buffer_has_identifier},
};
use metrics::{counter, describe_counter, describe_gauge, gauge};
use metrics_exporter_prometheus::PrometheusBuilder;
use miette::IntoDiagnostic;
use rdkafka::{
    consumer::{CommitMode, Consumer},
    message::Message,
};
use std::{net::SocketAddr, path::PathBuf};
use tokio::signal::unix::{SignalKind, signal};
use tracing::{error, info, info_span, warn};

/// Command-line interface for the trace-writer.
#[derive(Debug, Parser)]
#[clap(author, version = digital_muon_common::version!(), about)]
struct Cli {
    #[clap(flatten)]
    common_kafka_options: CommonKafkaOpts,

    /// Kafka consumer group.
    #[clap(long)]
    consumer_group: String,

    /// The Kafka topic that digitiser analog trace messages (`dat2`) are consumed from.
    #[clap(long)]
    trace_topic: String,

    /// Optional Kafka control topic.
    ///
    /// When set, `RunStart` messages cause a new HDF5 file to be opened and
    /// `RunStop` messages close the current file. `--output` is treated as a
    /// directory in this mode.
    ///
    /// When not set, a single HDF5 file is written continuously and `--output`
    /// is the full path to that file.
    #[clap(long)]
    control_topic: Option<String>,

    /// Output path.
    ///
    /// - **Continuous mode** (no `--control-topic`): full path to the HDF5 output file.
    /// - **Run-bounded mode** (`--control-topic` set): directory in which per-run HDF5
    ///   files are created.
    #[clap(long)]
    output: PathBuf,

    /// HDF5 chunk size (number of elements) used when creating resizable datasets.
    #[clap(long, default_value = "1024")]
    chunk_size: usize,

    /// Endpoint on which OpenMetrics flavour metrics are available.
    #[clap(long, env, default_value = "127.0.0.1:9090")]
    observability_address: SocketAddr,

    /// If set, OpenTelemetry traces are sent to this endpoint URL.
    /// Otherwise the standard tracing subscriber is used.
    #[clap(long)]
    otel_endpoint: Option<String>,

    /// OpenTelemetry `service.namespace` attribute. Used to distinguish
    /// parallel pipeline instances.
    #[clap(long, default_value = "")]
    otel_namespace: String,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    let args = Cli::parse();

    let tracer = init_tracer!(TracerOptions::new(
        args.otel_endpoint.as_deref(),
        args.otel_namespace.clone()
    ));

    let kafka_opts = &args.common_kafka_options;

    // Build the list of topics to subscribe to.
    let mut topics: Vec<&str> = vec![args.trace_topic.as_str()];
    if let Some(ct) = &args.control_topic {
        topics.push(ct.as_str());
    }

    let consumer = digital_muon_common::create_default_consumer(
        &kafka_opts.broker,
        &kafka_opts.username,
        &kafka_opts.password,
        &args.consumer_group,
        Some(&topics),
    )
    .into_diagnostic()?;

    // Install Prometheus metrics exporter.
    PrometheusBuilder::new()
        .with_http_listener(args.observability_address)
        .install()
        .into_diagnostic()?;

    describe_counter!(
        MESSAGES_RECEIVED,
        metrics::Unit::Count,
        "Number of messages received"
    );
    describe_counter!(
        MESSAGES_PROCESSED,
        metrics::Unit::Count,
        "Number of messages successfully processed"
    );
    describe_counter!(FAILURES, metrics::Unit::Count, "Number of failures");
    describe_gauge!(
        LAST_MESSAGE_TIMESTAMP,
        "GPS timestamp (ns) of the last received trace message per digitiser"
    );
    describe_gauge!(
        LAST_MESSAGE_FRAME_NUMBER,
        "Frame number of the last received trace message per digitiser"
    );

    component_info_metric("trace-writer");

    // Prepare the initial writer state.
    let mut writer: Option<TraceFileWriter> = if args.control_topic.is_none() {
        // Continuous mode: open the output file immediately.
        let w = TraceFileWriter::new(&args.output, args.chunk_size)
            .map_err(|e| miette::miette!("{e}"))?;
        info!("Opened HDF5 file for continuous writing: {:?}", args.output);
        Some(w)
    } else {
        // Run-bounded mode: make sure the output directory exists.
        std::fs::create_dir_all(&args.output).into_diagnostic()?;
        info!(
            "Run-bounded mode: HDF5 files will be written to {:?}",
            args.output
        );
        None
    };

    let mut sigint = signal(SignalKind::interrupt()).into_diagnostic()?;

    loop {
        tokio::select! {
            event = consumer.recv() => {
                match event {
                    Err(e) => warn!("Kafka error: {e}"),
                    Ok(msg) => {
                        let span = info_span!("message_received");
                        msg.headers()
                            .conditional_extract_to_span(tracer.use_otel(), &span);
                        let _guard = span.enter();

                        if let Some(payload) = msg.payload() {
                            if msg.topic() == args.trace_topic {
                                handle_trace_message(payload, writer.as_mut());
                            } else if args.control_topic.as_deref() == Some(msg.topic()) {
                                handle_control_message(
                                    payload,
                                    &mut writer,
                                    &args.output,
                                    args.chunk_size,
                                );
                            } else {
                                warn!("Message received on unexpected topic: {}", msg.topic());
                                counter!(
                                    MESSAGES_RECEIVED,
                                    &[messages_received::get_label(MessageKind::Unexpected)]
                                )
                                .increment(1);
                            }
                        }

                        if let Err(e) = consumer.commit_message(&msg, CommitMode::Async) {
                            error!("Failed to commit Kafka message offset: {e}");
                        }
                    }
                }
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, flushing and closing HDF5 file");
                if let Some(w) = writer.take()
                    && let Err(e) = w.close() {
                        error!("Failed to close HDF5 file on shutdown: {e}");
                    }
                return Ok(());
            }
        }
    }
}

/// Handles a message received on the trace topic.
///
/// Decodes it as a [`DigitizerAnalogTraceMessage`], updates metrics, and
/// appends the trace data to the current HDF5 file (if one is open).
#[tracing::instrument(skip_all, fields(payload_size = payload.len()))]
fn handle_trace_message(payload: &[u8], writer: Option<&mut TraceFileWriter>) {
    if !digitizer_analog_trace_message_buffer_has_identifier(payload) {
        warn!("Message on trace topic has unexpected identifier");
        counter!(
            MESSAGES_RECEIVED,
            &[messages_received::get_label(MessageKind::Unexpected)]
        )
        .increment(1);
        return;
    }

    counter!(
        MESSAGES_RECEIVED,
        &[messages_received::get_label(MessageKind::Trace)]
    )
    .increment(1);

    let trace = match root_as_digitizer_analog_trace_message(payload) {
        Ok(t) => t,
        Err(e) => {
            warn!("Failed to decode trace message: {e}");
            counter!(
                FAILURES,
                &[failures::get_label(FailureKind::UnableToDecodeMessage)]
            )
            .increment(1);
            return;
        }
    };

    // Update per-digitiser metrics.
    let did = trace.digitizer_id().to_string();

    if let Some(dt) = trace
        .metadata()
        .timestamp()
        .copied()
        .and_then(|t| DateTime::<Utc>::try_from(t).ok())
        && let Some(ns) = dt.timestamp_nanos_opt()
    {
        gauge!(
            LAST_MESSAGE_TIMESTAMP,
            &[
                messages_received::get_label(MessageKind::Trace),
                ("digitizer_id", did.clone())
            ]
        )
        .set(ns as f64);
    }
    gauge!(
        LAST_MESSAGE_FRAME_NUMBER,
        &[
            messages_received::get_label(MessageKind::Trace),
            ("digitizer_id", did)
        ]
    )
    .set(trace.metadata().frame_number() as f64);

    // Write to the open HDF5 file.
    let Some(w) = writer else {
        warn!(
            "Trace message (digitiser {}, frame {}) received but no HDF5 file is open \
             — is a RunStart message missing?",
            trace.digitizer_id(),
            trace.metadata().frame_number(),
        );
        counter!(
            FAILURES,
            &[failures::get_label(FailureKind::FileWriteFailed)]
        )
        .increment(1);
        return;
    };

    match w.write_trace_message(&trace) {
        Ok(()) => {
            counter!(
                MESSAGES_PROCESSED,
                &[messages_received::get_label(MessageKind::Trace)]
            )
            .increment(1);
        }
        Err(e) => {
            warn!("Failed to write trace message to HDF5 file: {e}");
            counter!(
                FAILURES,
                &[failures::get_label(FailureKind::FileWriteFailed)]
            )
            .increment(1);
        }
    }
}

/// Handles a message received on the control topic.
///
/// `RunStart` closes any currently open HDF5 file and opens a new one named
/// after the run. `RunStop` closes the current file.
#[tracing::instrument(skip_all, fields(payload_size = payload.len()))]
fn handle_control_message(
    payload: &[u8],
    writer: &mut Option<TraceFileWriter>,
    output_dir: &std::path::Path,
    chunk_size: usize,
) {
    if run_start_buffer_has_identifier(payload) {
        counter!(
            MESSAGES_RECEIVED,
            &[messages_received::get_label(MessageKind::RunStart)]
        )
        .increment(1);

        match root_as_run_start(payload) {
            Ok(run_start) => {
                // Close any file that is currently open.
                if let Some(old) = writer.take()
                    && let Err(e) = old.close()
                {
                    warn!("Failed to close previous HDF5 file before RunStart: {e}");
                }

                // Derive a safe file name from the run name.
                let run_name = run_start.run_name().unwrap_or("unknown_run");
                let safe_name: String = run_name
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '_' || c == '-' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let path = output_dir.join(format!("{safe_name}.h5"));

                info!("RunStart received (run={run_name:?}), creating HDF5 file: {path:?}");

                match TraceFileWriter::new(&path, chunk_size) {
                    Ok(w) => {
                        *writer = Some(w);
                        counter!(
                            MESSAGES_PROCESSED,
                            &[messages_received::get_label(MessageKind::RunStart)]
                        )
                        .increment(1);
                    }
                    Err(e) => {
                        warn!("Failed to create HDF5 file {path:?}: {e}");
                        counter!(
                            FAILURES,
                            &[failures::get_label(FailureKind::FileWriteFailed)]
                        )
                        .increment(1);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to decode RunStart message: {e}");
                counter!(
                    FAILURES,
                    &[failures::get_label(FailureKind::UnableToDecodeMessage)]
                )
                .increment(1);
            }
        }
    } else if run_stop_buffer_has_identifier(payload) {
        counter!(
            MESSAGES_RECEIVED,
            &[messages_received::get_label(MessageKind::RunStop)]
        )
        .increment(1);

        match root_as_run_stop(payload) {
            Ok(run_stop) => {
                let run_name = run_stop.run_name().unwrap_or("?");
                info!("RunStop received (run={run_name:?}), closing HDF5 file");

                match writer.take() {
                    Some(w) => match w.close() {
                        Ok(()) => {
                            counter!(
                                MESSAGES_PROCESSED,
                                &[messages_received::get_label(MessageKind::RunStop)]
                            )
                            .increment(1);
                        }
                        Err(e) => {
                            warn!("Failed to close HDF5 file on RunStop: {e}");
                            counter!(
                                FAILURES,
                                &[failures::get_label(FailureKind::FileWriteFailed)]
                            )
                            .increment(1);
                        }
                    },
                    None => warn!(
                        "RunStop received (run={run_name:?}) but no HDF5 file is currently open"
                    ),
                }
            }
            Err(e) => {
                warn!("Failed to decode RunStop message: {e}");
                counter!(
                    FAILURES,
                    &[failures::get_label(FailureKind::UnableToDecodeMessage)]
                )
                .increment(1);
            }
        }
    } else {
        warn!("Message on control topic has unexpected identifier");
        counter!(
            MESSAGES_RECEIVED,
            &[messages_received::get_label(MessageKind::Unexpected)]
        )
        .increment(1);
    }
}
