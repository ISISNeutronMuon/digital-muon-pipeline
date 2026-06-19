mod loader;
mod processing;

use std::path::PathBuf;

pub(crate) use loader::load_trace_file;
pub(crate) use processing::dispatch_trace_file;
use rand::seq::IteratorRandom;
use rdkafka::{ClientConfig, producer::FutureProducer};

use crate::Picoscope;


pub(crate) async fn read_picoscope_file(
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
