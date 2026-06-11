mod hdf5trace;
mod picoscope;

use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use digital_muon_common::{CommonKafkaOpts, DigitizerId, FrameNumber};
use digital_muon_streaming_types::flatbuffers::FlatBufferBuilder;
use hdf5::File;
use picoscope::{dispatch_trace_file, load_trace_file};
use rand::seq::IteratorRandom;
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
};
use std::{path::PathBuf, time::Duration};
use tracing::{debug, error, info};

use crate::hdf5trace::Hdf5Digitiser;

#[derive(Parser)]
#[clap(author, version = digital_muon_common::version!(), about)]
struct Cli {
    #[clap(flatten)]
    common_kafka_options: CommonKafkaOpts,

    /// The Kafka topic that trace messages will be produced to
    #[clap(long)]
    trace_topic: String,

    /// Relative path to the .trace file to be read
    #[clap(long)]
    file_name: PathBuf,

    #[command(subcommand)]
    mode: Mode,
}

#[derive(Clone, Parser)]
struct Run {
    /// The frame number to assign the message
    #[clap(long)]
    run_name: String,

    /// The frame number to assign the message
    #[clap(long)]
    instrument_name: String,

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
    /// The frame number to begin
    #[clap(long)]
    from_frame: Option<FrameNumber>,

    /// The frame number to assign the message
    #[clap(long)]
    to_frame: Option<FrameNumber>,

    /// The frame number to assign the message
    #[clap(short, long, value_delimiter = ',')]
    digitizer_id: Vec<DigitizerId>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    let kafka_opts = args.common_kafka_options;

    let client_config = digital_muon_common::generate_kafka_client_config(
        &kafka_opts.broker,
        &kafka_opts.username,
        &kafka_opts.password,
    );

    let producer: FutureProducer = client_config
        .create()
        .expect("Kafka Producer should be created");

    match args.mode {
        Mode::Picoscope(picoscope) => {
            read_picoscope_file(args.file_name, &producer, &args.trace_topic, picoscope).await
        }
        Mode::HDF5(hdf5) => {
            read_hdf5_file(args.file_name, &producer, &args.trace_topic, hdf5).await
        }
    }
}

async fn read_picoscope_file(
    file_name: PathBuf,
    producer: &FutureProducer,
    trace_topic: &str,
    args: Picoscope,
) {
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

async fn read_hdf5_file(
    file_name: PathBuf,
    producer: &FutureProducer,
    trace_topic: &str,
    args: Hdf5,
) {
    let file = File::open(file_name).unwrap();
    let mut digitisers = Hdf5Digitiser::open_from(file).unwrap();

    let mut fbb = FlatBufferBuilder::new();

    let num_frames = digitisers.iter().map(|d| d.get_num_frames()).max().unwrap();
    for index in 0..num_frames {
        if index % 10 == 1 {
            info!("index {index}");
        }
        for digitiser in &mut digitisers {
            digitiser.ensure_elements_cached(index);
        }
        for digitiser in &digitisers {
            digitiser
                .create_message(&mut fbb, index, 1000000000)
                .unwrap();

            let future_record = FutureRecord::to(trace_topic)
                .payload(fbb.finished_data())
                .key("");
            let timeout = Timeout::After(Duration::from_millis(6000));
            match producer.send(future_record, timeout).await {
                Ok(r) => debug!("Delivery: {:?}", r),
                Err(e) => error!("Delivery failed: {:?}", e.0),
            };
        }
    }
}
