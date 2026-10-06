use crate::DigitiserIdsOpts;
use chrono::{DateTime, Utc};
use digital_muon_common::DigitizerId;
use digital_muon_streaming_types::{
    aev2_frame_assembled_event_v2_generated::{
        frame_assembled_event_list_message_buffer_has_identifier,
        root_as_frame_assembled_event_list_message,
    },
    dat2_digitizer_analog_trace_v2_generated::{
        digitizer_analog_trace_message_buffer_has_identifier,
        root_as_digitizer_analog_trace_message,
    },
    dev2_digitizer_event_v2_generated::{
        digitizer_event_list_message_buffer_has_identifier, root_as_digitizer_event_list_message,
    },
};
use miette::IntoDiagnostic;
use rdkafka::{
    Message,
    consumer::{Consumer, StreamConsumer},
};
use std::collections::{BTreeMap, BTreeSet};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
struct DigitiserRecord {
    keys: BTreeSet<String>,
    partitions: BTreeSet<i32>,
    message_count: u64,
    first_seen: DateTime<Utc>,
    last_seen: DateTime<Utc>,
}

impl DigitiserRecord {
    fn new(key: String, partition: i32, now: DateTime<Utc>) -> Self {
        let mut keys = BTreeSet::new();
        keys.insert(key);
        let mut partitions = BTreeSet::new();
        partitions.insert(partition);
        Self {
            keys,
            partitions,
            message_count: 1,
            first_seen: now,
            last_seen: now,
        }
    }

    fn record_occurrence(&mut self, key: String, partition: i32, now: DateTime<Utc>) {
        self.keys.insert(key);
        self.partitions.insert(partition);
        self.message_count += 1;
        self.last_seen = now;
    }
}

pub(crate) async fn run(args: DigitiserIdsOpts) -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    let kafka_opts = &args.common.common_kafka_options;

    let mut client_config = digital_muon_common::generate_kafka_client_config(
        &kafka_opts.broker,
        &kafka_opts.username,
        &kafka_opts.password,
    );
    client_config
        .set("group.id", &args.common.consumer_group)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "false");

    if args.from_beginning {
        client_config.set("auto.offset.reset", "earliest");
    } else {
        client_config.set("auto.offset.reset", "latest");
    }

    let consumer: StreamConsumer = client_config.create().into_diagnostic()?;
    consumer
        .subscribe(&[&args.common.topic])
        .into_diagnostic()?;

    let mut records: BTreeMap<DigitizerId, DigitiserRecord> = BTreeMap::new();

    let mut interval_timer = args
        .report_interval
        .map(|secs| tokio::time::interval(std::time::Duration::from_secs(secs)));

    info!(
        "Listening for messages on topic \"{}\" (group: \"{}\")... Press Ctrl-C to finish and view report.",
        args.common.topic, args.common.consumer_group
    );

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Interrupt received. Generating report...");
                break;
            }
            _ = async {
                match &mut interval_timer {
                    Some(timer) => {
                        timer.tick().await;
                    }
                    None => std::future::pending().await,
                }
            } => {
                print_report(&args.common.topic, &records);
            }
            msg_res = consumer.recv() => {
                match msg_res {
                    Err(e) => warn!("Kafka error: {e}"),
                    Ok(msg) => {
                        let partition = msg.partition();
                        let key_str = format_kafka_key(msg.key());

                        if let Some(payload) = msg.payload() {
                            let ids = extract_digitiser_ids(payload);
                            let now = Utc::now();
                            for id in ids {
                                match records.get_mut(&id) {
                                    Some(record) => {
                                        record.record_occurrence(key_str.clone(), partition, now);
                                    }
                                    None => {
                                        info!(
                                            "New digitiser seen: ID {}, Kafka key: \"{}\", partition: {}",
                                            id, key_str, partition
                                        );
                                        records.insert(
                                            id,
                                            DigitiserRecord::new(key_str.clone(), partition, now),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    print_report(&args.common.topic, &records);
    Ok(())
}

fn format_kafka_key(key_bytes: Option<&[u8]>) -> String {
    match key_bytes {
        None => "<none>".to_string(),
        Some([]) => "<empty>".to_string(),
        Some(k) => String::from_utf8_lossy(k).into_owned(),
    }
}

fn extract_digitiser_ids(payload: &[u8]) -> Vec<DigitizerId> {
    if digitizer_analog_trace_message_buffer_has_identifier(payload) {
        match root_as_digitizer_analog_trace_message(payload) {
            Ok(msg) => vec![msg.digitizer_id()],
            Err(e) => {
                warn!("Failed to parse dat2 trace message: {e}");
                Vec::new()
            }
        }
    } else if digitizer_event_list_message_buffer_has_identifier(payload) {
        match root_as_digitizer_event_list_message(payload) {
            Ok(msg) => vec![msg.digitizer_id()],
            Err(e) => {
                warn!("Failed to parse dev2 event list message: {e}");
                Vec::new()
            }
        }
    } else if frame_assembled_event_list_message_buffer_has_identifier(payload) {
        match root_as_frame_assembled_event_list_message(payload) {
            Ok(msg) => msg
                .digitizers_present()
                .map(|v| v.iter().collect())
                .unwrap_or_default(),
            Err(e) => {
                warn!("Failed to parse aev2 assembled event message: {e}");
                Vec::new()
            }
        }
    } else {
        debug!("Message payload does not match any known digitiser flatbuffer identifier");
        Vec::new()
    }
}

fn print_report(topic: &str, records: &BTreeMap<DigitizerId, DigitiserRecord>) {
    println!("\n============================= Digitiser Report =============================");
    println!("Topic: {topic}");
    println!("Total unique digitisers: {}", records.len());
    let total_messages: u64 = records.values().map(|r| r.message_count).sum();
    println!("Total digitiser messages: {total_messages}");

    if records.is_empty() {
        println!("No digitisers were observed.");
        println!("============================================================================\n");
        return;
    }

    let key_width = records
        .values()
        .map(|r| r.keys.iter().cloned().collect::<Vec<_>>().join(", ").len())
        .max()
        .unwrap_or(18)
        .max(18);

    let part_width = records
        .values()
        .map(|r| {
            r.partitions
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ")
                .len()
        })
        .max()
        .unwrap_or(10)
        .max(10);

    println!(
        "\n{:<12} | {:<key_width$} | {:<part_width$} | {:<10} | {:<20} | {:<20}",
        "Digitiser ID", "Kafka Message Keys", "Partitions", "Messages", "First Seen", "Last Seen"
    );
    println!(
        "{:-<12}-+-{:-<key_width$}-+-{:-<part_width$}-+-{:-<10}-+-{:-<20}-+-{:-<20}",
        "", "", "", "", "", ""
    );

    for (id, rec) in records {
        let keys = rec.keys.iter().cloned().collect::<Vec<_>>().join(", ");
        let partitions = rec
            .partitions
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let first_seen = rec.first_seen.format("%Y-%m-%d %H:%M:%S UTC").to_string();
        let last_seen = rec.last_seen.format("%Y-%m-%d %H:%M:%S UTC").to_string();

        println!(
            "{:<12} | {:<key_width$} | {:<part_width$} | {:<10} | {:<20} | {:<20}",
            id, keys, partitions, rec.message_count, first_seen, last_seen
        );
    }
    println!("============================================================================\n");
}
