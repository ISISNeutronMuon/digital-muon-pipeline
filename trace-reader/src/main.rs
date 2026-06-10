mod picoscope;

use chrono::{DateTime, Utc};
use digital_muon_streaming_types::{dat2_digitizer_analog_trace_v2_generated::ChannelTrace, frame_metadata_v2_generated::GpsTime};
use hdf5::{Dataset, File, Group, types::{VarLenArray, VarLenUnicode}};
use picoscope::{load_trace_file, dispatch_trace_file};
use clap::{Parser, Subcommand};
use digital_muon_common::{Channel, CommonKafkaOpts, DigitizerId, FrameNumber, Intensity};
use rand::seq::IteratorRandom;
use rdkafka::{producer::FutureProducer};
use std::path::PathBuf;
use ndarray::s;

#[derive(Parser)]
#[clap(author, version = digital_muon_common::version!(), about)]
struct Cli {
    #[clap(flatten)]
    common_kafka_options: CommonKafkaOpts,

    /// Kafka consumer group
    #[clap(long)]
    consumer_group: String,

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
    digitizer_id: DigitizerId,
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
        Mode::Picoscope(picoscope) => read_picoscope_file(args.file_name, &producer, &args.trace_topic, picoscope).await,
        Mode::HDF5(hdf5) => read_hdf5_file(args.file_name, &producer, &args.trace_topic, hdf5).await,
    }
}

async fn read_picoscope_file(file_name: PathBuf, producer: &FutureProducer, trace_topic: &str, args: Picoscope) {
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

const DIGITISER: &'static str = "digitiser";
const CHANNEL: &'static str = "channel";

struct Hdf5Digitiser {
    id: DigitizerId,
    periods: Dataset,
    frame_numbers: Dataset,
    timestamps: Dataset,
    channels: Vec<Hdf5Channel>
}

impl Hdf5Digitiser {
    fn build_digitiser_message(&self, from_frame_number: FrameNumber, to_frame_number: FrameNumber) -> Result<(), String> {
        
        let slice = self.frame_numbers.read_1d::<FrameNumber>().unwrap();
        let from_index = slice.iter().enumerate()
            .find_map(|(index, &frame)|(frame == from_frame_number).then_some(index))
            .ok_or_else(||"Starting frame not present")?;
        let to_index = slice.iter().enumerate()
            .find_map(|(index, &frame)|(frame == to_frame_number).then_some(index))
            .ok_or_else(||"Ending frame not present")?;
        for index in from_index..to_index {
            let array = self.frame_numbers.read_1d().unwrap();
            let frame_number : FrameNumber = *array.get([index]).unwrap();
            let array = self.periods.read_1d().unwrap();
            let period : u64 = *array.get([index]).unwrap();
            let array = self.timestamps.read_1d::<VarLenUnicode>().unwrap();
            let timestamp = array.get([index]).unwrap().clone();
            for channel in &self.channels {
                let array = channel.trace.read_1d::<VarLenArray<Intensity>>().unwrap();
                let trace = array.get([index]).unwrap().clone();
            }
        }
        Ok(())
    }
}

struct Hdf5Channel {
    channel: Channel,
    trace: Dataset
}

async fn read_hdf5_file(file_name: PathBuf, producer: &FutureProducer, trace_topic: &str, args: Hdf5) {
    let file = File::open(file_name).unwrap();
    let digitisers = get_group_structure(file);
}

fn get_group_structure(file: File) -> Vec<Hdf5Digitiser> {
    let mut digitisers = Vec::<Hdf5Digitiser>::new();
    for group in file.groups().unwrap() {
        let name = group.name();
        let group_name = name.split('_').collect::<Vec<_>>();
        if group_name.len() < 1 {
            break;
        }
        if group_name.get(0).unwrap() != &DIGITISER {
            break;
        }
        let digitiser_id : DigitizerId = group_name.get(1).unwrap().parse().unwrap();

        let periods = group.dataset("period").unwrap();
        let frame_numbers = group.dataset("frame_number").unwrap();
        let timestamps = group.dataset("timestamp").unwrap();

        let mut channels = Vec::<Hdf5Channel>::new();

        for dataset in group.datasets().unwrap() {
            let name = dataset.name();
            let dataset_name = name.split('_').collect::<Vec<_>>();
            if dataset_name.len() < 1 {
                break;
            }
            if dataset_name.get(0).unwrap() != &CHANNEL {
                break;
            }

            let channel : Channel = dataset_name.get(1).unwrap().parse().unwrap();
            channels.push(Hdf5Channel{ channel, trace: dataset });

        }
        digitisers.push(Hdf5Digitiser { id: digitiser_id, periods, frame_numbers, timestamps, channels });
    }
    digitisers
}