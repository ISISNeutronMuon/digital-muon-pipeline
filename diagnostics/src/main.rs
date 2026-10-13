mod daq_trace;
mod digitiser_ids;
mod duplicate_detector;
mod flatbuffer_decode;
mod kafka_tail;

use clap::{Args, Parser, Subcommand};
use digital_muon_common::CommonKafkaOpts;
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::{
    digitizer_analog_trace_message_buffer_has_identifier, root_as_digitizer_analog_trace_message,
};
use isis_streaming_data_types::flatbuffers_generated::{
    run_start_pl72::{root_as_run_start, run_start_buffer_has_identifier},
    run_stop_6s4t::{root_as_run_stop, run_stop_buffer_has_identifier},
};
use tracing::{info, warn};

#[derive(Debug, Parser)]
#[clap(author, version = digital_muon_common::version!(), about)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Provides metrics regarding data transmission from the digitisers via Kafka.
    #[clap(name = "daq-trace")]
    DaqTrace(DaqTraceOpts),

    /// Run message dumping tool.
    #[clap(name = "kafka-tail")]
    KafkaTail(CommonOpts),

    /// Collect and report unique digitiser IDs, their Kafka message keys, and partitions.
    #[clap(name = "digitiser-ids", visible_aliases = ["digitisers", "digitizer-ids", "digitizers"])]
    DigitiserIds(DigitiserIdsOpts),

    /// Report instances where multiple of the same digitiser ID appear for the same frame.
    #[clap(
        name = "duplicate-digitisers",
        visible_aliases = ["duplicate-frames", "digitiser-duplicates", "duplicate-digitizers", "duplicates"]
    )]
    DuplicateDigitisers(DuplicateDigitisersOpts),

    /// Decode a flatbuffer encoded message.
    ///
    /// Shows a basic summary of the following message types:
    /// Digitiser trace, Run start, Run stop
    ///
    /// Encoded message must be provided in hexadecimal representation with a space between each
    /// byte (as per the format used by Redpanda Console).
    #[clap(name = "decode")]
    FlatbufferDecode,
}

#[derive(Debug, Args)]
pub(crate) struct CommonOpts {
    #[clap(flatten)]
    common_kafka_options: CommonKafkaOpts,

    /// Kafka consumer group
    #[clap(long = "group")]
    consumer_group: String,

    /// The Kafka topic to consume messages from
    #[clap(long)]
    topic: String,
}

#[derive(Debug, Args)]
pub(crate) struct DaqTraceOpts {
    /// The interval at which the message rate is calculated in seconds.
    #[clap(long, default_value_t = 5)]
    message_rate_interval: u64,

    #[clap(flatten)]
    common: CommonOpts,
}

#[derive(Debug, Args)]
pub(crate) struct DigitiserIdsOpts {
    /// Consume from the beginning of the topic (earliest offset) rather than the latest
    #[clap(long)]
    pub(crate) from_beginning: bool,

    /// Optional periodic interval in seconds to print summary reports while running
    #[clap(long)]
    pub(crate) report_interval: Option<u64>,

    #[clap(flatten)]
    pub(crate) common: CommonOpts,
}

#[derive(Debug, Args)]
pub(crate) struct DuplicateDigitisersOpts {
    /// Consume from the beginning of the topic (earliest offset) rather than the latest
    #[clap(long)]
    pub(crate) from_beginning: bool,

    /// Optional periodic interval in seconds to print summary reports while running
    #[clap(long)]
    pub(crate) report_interval: Option<u64>,

    /// Maximum number of active frames to retain in cache for duplicate detection (0 for unlimited)
    #[clap(long, default_value_t = 10000)]
    pub(crate) max_frames: usize,

    #[clap(flatten)]
    pub(crate) common: CommonOpts,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::DaqTrace(args) => daq_trace::run(args).await,
        Commands::KafkaTail(args) => kafka_tail::run(args).await,
        Commands::FlatbufferDecode => flatbuffer_decode::run().await,
        Commands::DigitiserIds(args) => digitiser_ids::run(args).await,
        Commands::DuplicateDigitisers(args) => duplicate_detector::run(args).await,
    }
}

fn decode_and_print(payload: &[u8]) {
    if digitizer_analog_trace_message_buffer_has_identifier(payload) {
        match root_as_digitizer_analog_trace_message(payload) {
            Ok(data) => {
                info!(
                    "Trace packet: dig. ID: {}, metadata: {:?}",
                    data.digitizer_id(),
                    data.metadata()
                );
            }
            Err(e) => {
                warn!("Failed to parse message: {}", e);
            }
        }
    } else if run_start_buffer_has_identifier(payload) {
        match root_as_run_start(payload) {
            Ok(data) => {
                info!(
                    "Run start: run name: {:?}, start time: {}",
                    data.run_name(),
                    data.start_time()
                );
            }
            Err(e) => {
                warn!("Failed to parse message: {}", e);
            }
        }
    } else if run_stop_buffer_has_identifier(payload) {
        match root_as_run_stop(payload) {
            Ok(data) => {
                info!("Run stop: run name: {:?}", data.run_name());
            }
            Err(e) => {
                warn!("Failed to parse message: {}", e);
            }
        }
    } else {
        // TODO: other message types
        warn!("Unexpected message type");
    }
}
