mod hdf5trace;
mod picoscope;

use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use crate::{hdf5trace::read_hdf5_file, picoscope::read_picoscope_file};
use digital_muon_common::{CommonKafkaOpts, DigitizerId, FrameNumber, init_tracer, tracer::{TracerEngine, TracerOptions}};
use std::path::PathBuf;

#[derive(Parser)]
#[clap(author, version = digital_muon_common::version!(), about)]
struct Cli {
    #[clap(flatten)]
    common_kafka_options: CommonKafkaOpts,

    /// If set, then OpenTelemetry data is sent to the URL specified, otherwise the standard tracing subscriber is used.
    #[clap(long)]
    otel_endpoint: Option<String>,

    /// All OpenTelemetry spans are emitted with this as the "service.namespace" property. Can be used to track different instances of the pipeline running in parallel.
    #[clap(long, default_value = "")]
    otel_namespace: String,

    /// The Kafka topic that trace messages will be produced to
    #[clap(long)]
    trace_topic: String,

    /// Relative path to the .trace file to be read
    #[clap(long)]
    file_name: PathBuf,

    /// Relative path to the .trace file to be read
    #[clap(flatten)]
    run: Run,

    #[command(subcommand)]
    mode: Mode,
}

#[derive(Clone, Parser)]
struct Run {
    /// The frame number to assign the message
    #[clap(long)]
    run_name: Option<String>,

    /// The frame number to assign the message
    #[clap(long)]
    instrument_name: Option<String>,

    /// The frame number to assign the message
    #[clap(long)]
    run_start_time: Option<DateTime<Utc>>,

    /// The frame number to assign the message
    #[clap(long)]
    run_stop_time: Option<DateTime<Utc>>,
}

#[derive(Clone, Subcommand)]
enum Mode {
    /// Run in single shot mode, output a single frame then exit
    Picoscope(Picoscope),

    /// Run in continuous mode, outputting one frame every `frame-time` milliseconds
    HDF5(Hdf5),
}

#[derive(Clone, Parser)]
struct Picoscope {
    /// The frame number to assign the message
    #[clap(long, default_value = "0")]
    frame_number: FrameNumber,

    /// The digitizer ID to assign the message
    #[clap(long, default_value = "0")]
    digitizer_id: DigitizerId,

    /// The number of trace events to read. If zero, then all trace events are read
    #[clap(long, default_value = "1")]
    number_of_trace_events: usize,

    /// If set, then trace events are sampled randomly with replacement, if not set then trace events are read in order
    #[clap(long, default_value = "false")]
    random_sample: bool,
}

#[derive(Clone, Parser)]
struct Hdf5 {
    /// If true, print a summary of the contents and exit.
    #[clap(long)]
    summary_only: bool,

    /// If present the index to begin the run with, otherwise, derived from `from_frame_number`.
    #[clap(long)]
    from_index: Option<usize>,

    /// If present the index to end the run with, otherwise, derived from `to_frame_number`.
    #[clap(long)]
    to_index: Option<usize>,

    /// Only if `from_index` is not present, If present, the frame to begin the run with, otherwise, starts at 0.
    #[clap(long)]
    from_frame_number: Option<FrameNumber>,

    /// Only if `to_index` is not present, If present, the frame to end the run with, otherwise, ends at the last frame.
    #[clap(long)]
    to_frame_number: Option<FrameNumber>,

    /// If non-empty, only emit the given digitiser ids, otherwise emit all.
    #[clap(short, long, value_delimiter = ',')]
    digitizer_id: Vec<DigitizerId>,

    /// If set, all timestamps are shifted to today's date.
    #[clap(long)]
    shift_to_today: bool,

    /// If set, load the datasets in chunks of this size, otherwise use the given chunk size in the file.
    #[clap(long)]
    cache_size: Option<usize>,
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    let _tracer = init_tracer!(TracerOptions::new(
        args.otel_endpoint.as_deref(),
        args.otel_namespace.clone()
    ));

    let kafka_opts = args.common_kafka_options;

    let client_config = digital_muon_common::generate_kafka_client_config(
        &kafka_opts.broker,
        &kafka_opts.username,
        &kafka_opts.password,
    );

    match args.mode {
        Mode::Picoscope(picoscope) => {
            read_picoscope_file(args.file_name, &client_config, &args.trace_topic, picoscope).await
        }
        Mode::HDF5(hdf5) => {
            read_hdf5_file(args.file_name, &client_config, &args.trace_topic, hdf5).await
        }
    }
}
