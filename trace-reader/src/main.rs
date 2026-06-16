mod hdf5trace;
mod picoscope;

use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use digital_muon_common::{CommonKafkaOpts, DigitizerId, FrameNumber, init_tracer, spanned::{Spanned, SpanWrapper}, tracer::{TracerEngine, TracerOptions}};
use digital_muon_streaming_types::flatbuffers::FlatBufferBuilder;
use hdf5::{File, OpenMode, file::LogFlags};
use picoscope::{dispatch_trace_file, load_trace_file};
use rand::seq::IteratorRandom;
use rayon::iter::{ParallelIterator, IntoParallelRefMutIterator};
use rdkafka::{
    ClientConfig, producer::{BaseRecord, DefaultProducerContext, FutureProducer, ThreadedProducer}
};
use std::path::PathBuf;
use tracing::info_span;

use crate::hdf5trace::{Hdf5Digitiser, HDF5Config};

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

async fn read_picoscope_file(
    file_name: PathBuf,
    client_config: &ClientConfig,
    trace_topic: &str,
    args: Picoscope,
) {
    let producer: FutureProducer = client_config
        .create()
        .expect("Kafka Producer should be created");

    let trace_file = load_trace_file(file_name).expect("Trace File should load");
    let total_trace_events = trace_file.get_number_of_trace_events();
    let num_trace_events = if args.number_of_trace_events == 0 {
        total_trace_events
    } else {
        args.number_of_trace_events
    };

    let trace_event_indices: Vec<_> = if args.random_sample {
        (0..num_trace_events)
            .map(|_| {
                (0..num_trace_events)
                    .choose(&mut rand::rng())
                    .unwrap_or_default()
            })
            .collect()
    } else {
        (0..num_trace_events)
            .cycle()
            .take(num_trace_events)
            .collect()
    };

    dispatch_trace_file(
        trace_file,
        trace_event_indices,
        args.frame_number,
        args.digitizer_id,
        &producer,
        trace_topic,
        6000,
    )
    .await
    .expect("Trace File should be dispatched to Kafka");
}

struct DigitiserReader {
    digitiser: Hdf5Digitiser,
    from_index: usize,
    to_index: usize,
    producer: ThreadedProducer<DefaultProducerContext>
}

impl DigitiserReader {
    fn new(client_config: &ClientConfig, args: &Hdf5, digitiser: Hdf5Digitiser) -> Self {
        let from_index = args.from_index
            .unwrap_or(args.from_frame_number
                .and_then(|frame_number|digitiser.get_index_from_frame_number(frame_number))
                .unwrap_or_default()
            );
        let to_index = args.to_index
            .unwrap_or(args.to_frame_number
                .and_then(|frame_number|digitiser.get_index_from_frame_number(frame_number))
                .unwrap_or(digitiser.get_num_frames() - 1)
            );
        let producer = client_config.create().unwrap();
        Self {
            digitiser,
            from_index,
            to_index,
            producer
        }
    }
}

async fn read_hdf5_file(
    file_name: PathBuf,
    client_config: &ClientConfig,
    trace_topic: &str,
    args: Hdf5,
) {
    let file = {
        let mut file_builder = File::with_options();
        file_builder.fapl().stdio();//log_options(Some("mylog"), LogFlags::union(LogFlags::LOC_IO, LogFlags::TIME_READ), 0);//
        file_builder.open_as(file_name, OpenMode::Read).unwrap()
    };


    let config = HDF5Config {
        timestamp_as_rfc3339: file.attr("config_timestamp_as_rfc3339")
            .and_then(|config|config.read_scalar::<bool>())
            .unwrap_or(true),
        multiple_channel_datasets: file.attr("config_multiple_channel_datasets")
            .and_then(|config|config.read_scalar::<bool>())
            .unwrap_or(true),
        
    };
    
    let mut digitisers = Hdf5Digitiser::open_from(file, config).unwrap()
        .into_iter()
        .map(|digitiser|DigitiserReader::new(client_config, &args, digitiser))
        .collect::<Vec<_>>();

    if args.summary_only {
        for digitiser in digitisers.iter_mut() {
            digitiser.digitiser.output_summary();
        }
    } else {
        let num_indices = digitisers.iter().map(|digitiser| digitiser.to_index - digitiser.from_index).min().unwrap();
        for index in 0..num_indices {
            read_hdf5_frame(&mut digitisers, index, trace_topic, &args).await;
        }
    }
}

#[tracing::instrument(skip_all)]
async fn read_hdf5_frame(
    digitisers: &mut [DigitiserReader],
    index: usize,
    trace_topic: &str,
    args: &Hdf5
) {
    let mut spanned_digitisers = digitisers
        .iter_mut()
        .map(|digitiser|SpanWrapper::<_>::new(info_span!("Digitiser"), digitiser))
        .collect::<Vec<_>>();

    spanned_digitisers.iter_mut()
        .for_each(|spanned_digitiser| {
            let digitiser_index = index + spanned_digitiser.from_index;
            let span = spanned_digitiser
                .span()
                .get()
                .expect("Digitiser has span")
                .clone();
            span.in_scope(||
                spanned_digitiser.digitiser.ensure_elements_cached(digitiser_index)
            );
        }
    );

    spanned_digitisers
        .par_iter_mut()
        .map(|spanned_digitiser| {
            let mut fbb = FlatBufferBuilder::new();
            let span = spanned_digitiser
                .span()
                .get()
                .expect("Digitiser has span");
            span.in_scope(|| {
                    spanned_digitiser.digitiser
                        .create_message(&mut fbb, index, 1000000000, args.shift_to_today)
                        .unwrap();
            });
            let send_span = info_span!(parent: span.clone(), "Send");
            let _sguard = send_span.enter();
            let future_record = BaseRecord::to(trace_topic)
                .payload(fbb.finished_data())
                .key("");
            
            let mut result = spanned_digitiser.producer.send(future_record);
            while let Err((_, future_record)) = result {
                result = spanned_digitiser.producer.send(future_record);
            }
            (send_span.clone(), result.unwrap())
    }).collect::<Vec<_>>();
}
