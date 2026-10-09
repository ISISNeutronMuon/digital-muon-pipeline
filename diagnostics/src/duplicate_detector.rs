use crate::DuplicateDigitisersOpts;
use chrono::{DateTime, Utc};
use digital_muon_common::DigitizerId;
use digital_muon_streaming_types::{
    dat2_digitizer_analog_trace_v2_generated::{
        digitizer_analog_trace_message_buffer_has_identifier,
        root_as_digitizer_analog_trace_message,
    },
    dev2_digitizer_event_v2_generated::{
        digitizer_event_list_message_buffer_has_identifier, root_as_digitizer_event_list_message,
    },
    frame_metadata_v2_generated::GpsTime,
};
use miette::IntoDiagnostic;
use rdkafka::{
    Message,
    consumer::{Consumer, StreamConsumer},
};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use tracing::{debug, info, warn};

const MAX_DETAILED_EVENTS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawGpsTime {
    pub year: u8,
    pub day: u16,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millisecond: u16,
    pub microsecond: u16,
    pub nanosecond: u16,
}

impl From<&GpsTime> for RawGpsTime {
    fn from(t: &GpsTime) -> Self {
        Self {
            year: t.year(),
            day: t.day(),
            hour: t.hour(),
            minute: t.minute(),
            second: t.second(),
            millisecond: t.millisecond(),
            microsecond: t.microsecond(),
            nanosecond: t.nanosecond(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MessageTimestamp {
    Utc(DateTime<Utc>),
    Raw(RawGpsTime),
}

impl MessageTimestamp {
    pub fn from_gps_time(gps: &GpsTime) -> Self {
        match (*gps).try_into() {
            Ok(utc) => MessageTimestamp::Utc(utc),
            Err(_) => MessageTimestamp::Raw(RawGpsTime::from(gps)),
        }
    }
}

impl std::fmt::Display for MessageTimestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageTimestamp::Utc(dt) => write!(f, "{}", dt.format("%Y-%m-%d %H:%M:%S%.9f UTC")),
            MessageTimestamp::Raw(raw) => write!(
                f,
                "raw GPS (y:{}, d:{}, h:{}, m:{}, s:{}, ms:{}, us:{}, ns:{})",
                raw.year,
                raw.day,
                raw.hour,
                raw.minute,
                raw.second,
                raw.millisecond,
                raw.microsecond,
                raw.nanosecond
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MessageRecord {
    pub digitiser_id: DigitizerId,
    pub frame_number: u32,
    pub timestamp: Option<MessageTimestamp>,
    pub message_type: &'static str,
    pub partition: i32,
    pub offset: i64,
    pub key: String,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchReason {
    FrameNumberAndTimestamp,
    FrameNumberOnly,
    TimestampOnly,
    FrameMerge,
}

impl std::fmt::Display for MatchReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchReason::FrameNumberAndTimestamp => {
                write!(f, "both frame number and timestamp match")
            }
            MatchReason::FrameNumberOnly => write!(f, "same frame number"),
            MatchReason::TimestampOnly => write!(f, "same timestamp"),
            MatchReason::FrameMerge => write!(f, "frame merge collision"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DuplicateEvent {
    pub digitiser_id: DigitizerId,
    pub incoming: MessageRecord,
    pub conflicting: MessageRecord,
    pub match_reason: MatchReason,
}

type FrameId = u64;

#[derive(Debug, Clone)]
struct FrameState {
    frame_numbers: BTreeSet<u32>,
    timestamps: BTreeSet<MessageTimestamp>,
    digitisers: BTreeMap<DigitizerId, Vec<MessageRecord>>,
}

#[derive(Debug, Default, Clone)]
pub struct DuplicateStats {
    pub total_messages: u64,
    pub total_duplicates: u64,
    pub per_digitiser_messages: BTreeMap<DigitizerId, u64>,
    pub per_digitiser_duplicates: BTreeMap<DigitizerId, u64>,
    pub duplicate_events: Vec<DuplicateEvent>,
}

pub struct DuplicateDetector {
    next_frame_id: FrameId,
    frames: HashMap<FrameId, FrameState>,
    frame_by_number: HashMap<u32, FrameId>,
    frame_by_timestamp: HashMap<MessageTimestamp, FrameId>,
    insertion_order: VecDeque<FrameId>,
    max_frames: usize,
    stats: DuplicateStats,
}

impl DuplicateDetector {
    pub fn new(max_frames: usize) -> Self {
        Self {
            next_frame_id: 1,
            frames: HashMap::new(),
            frame_by_number: HashMap::new(),
            frame_by_timestamp: HashMap::new(),
            insertion_order: VecDeque::new(),
            max_frames,
            stats: DuplicateStats::default(),
        }
    }

    #[allow(dead_code)]
    pub fn stats(&self) -> &DuplicateStats {
        &self.stats
    }

    #[allow(dead_code)]
    pub fn active_frames_count(&self) -> usize {
        self.frames.len()
    }

    pub fn process_message(&mut self, record: MessageRecord) -> Vec<DuplicateEvent> {
        self.stats.total_messages += 1;
        *self
            .stats
            .per_digitiser_messages
            .entry(record.digitiser_id)
            .or_insert(0) += 1;

        let match_by_fn = self.frame_by_number.get(&record.frame_number).copied();
        let match_by_ts = record
            .timestamp
            .and_then(|ts| self.frame_by_timestamp.get(&ts).copied());

        let mut duplicates = Vec::new();

        match (match_by_fn, match_by_ts) {
            (None, None) => {
                let frame_id = self.next_frame_id;
                self.next_frame_id += 1;

                let mut frame_numbers = BTreeSet::new();
                frame_numbers.insert(record.frame_number);
                self.frame_by_number.insert(record.frame_number, frame_id);

                let mut timestamps = BTreeSet::new();
                if let Some(ts) = record.timestamp {
                    timestamps.insert(ts);
                    self.frame_by_timestamp.insert(ts, frame_id);
                }

                let mut digitisers = BTreeMap::new();
                let did = record.digitiser_id;
                digitisers.insert(did, vec![record]);

                self.frames.insert(
                    frame_id,
                    FrameState {
                        frame_numbers,
                        timestamps,
                        digitisers,
                    },
                );
                self.insertion_order.push_back(frame_id);
                self.evict_if_needed();
            }
            (Some(id1), None) => {
                self.handle_single_match(
                    id1,
                    record,
                    MatchReason::FrameNumberOnly,
                    &mut duplicates,
                );
            }
            (None, Some(id2)) => {
                self.handle_single_match(id2, record, MatchReason::TimestampOnly, &mut duplicates);
            }
            (Some(id1), Some(id2)) if id1 == id2 => {
                self.handle_single_match(
                    id1,
                    record,
                    MatchReason::FrameNumberAndTimestamp,
                    &mut duplicates,
                );
            }
            (Some(id1), Some(id2)) => {
                self.handle_merged_match(id1, id2, record, &mut duplicates);
            }
        }

        duplicates
    }

    fn handle_single_match(
        &mut self,
        frame_id: FrameId,
        record: MessageRecord,
        match_reason: MatchReason,
        duplicates: &mut Vec<DuplicateEvent>,
    ) {
        let mut dup_event = None;

        if let Some(frame) = self.frames.get_mut(&frame_id) {
            frame.frame_numbers.insert(record.frame_number);

            if let Some(ts) = record.timestamp {
                frame.timestamps.insert(ts);
            }

            if let Some(existing_msgs) = frame.digitisers.get(&record.digitiser_id)
                && let Some(conflicting) = existing_msgs.first().cloned()
            {
                dup_event = Some(DuplicateEvent {
                    digitiser_id: record.digitiser_id,
                    incoming: record.clone(),
                    conflicting,
                    match_reason,
                });
            }

            frame
                .digitisers
                .entry(record.digitiser_id)
                .or_default()
                .push(record.clone());
        }

        self.frame_by_number.insert(record.frame_number, frame_id);
        if let Some(ts) = record.timestamp {
            self.frame_by_timestamp.insert(ts, frame_id);
        }

        if let Some(dup) = dup_event {
            self.record_duplicate(dup.clone());
            duplicates.push(dup);
        }
    }

    fn handle_merged_match(
        &mut self,
        id1: FrameId,
        id2: FrameId,
        record: MessageRecord,
        duplicates: &mut Vec<DuplicateEvent>,
    ) {
        let frame2_opt = self.frames.remove(&id2);
        if let Some(frame2) = frame2_opt {
            let mut new_dups = Vec::new();
            let mut fns_to_repoint = Vec::new();
            let mut tss_to_repoint = Vec::new();

            if let Some(frame1) = self.frames.get_mut(&id1) {
                for fn_num in &frame2.frame_numbers {
                    frame1.frame_numbers.insert(*fn_num);
                    fns_to_repoint.push(*fn_num);
                }
                for ts in &frame2.timestamps {
                    frame1.timestamps.insert(*ts);
                    tss_to_repoint.push(*ts);
                }

                for (did, msgs) in frame2.digitisers {
                    if let Some(existing_msgs) = frame1.digitisers.get(&did)
                        && let Some(conflicting) = existing_msgs.first().cloned()
                    {
                        for m in &msgs {
                            new_dups.push(DuplicateEvent {
                                digitiser_id: did,
                                incoming: m.clone(),
                                conflicting: conflicting.clone(),
                                match_reason: MatchReason::FrameMerge,
                            });
                        }
                    }
                    frame1.digitisers.entry(did).or_default().extend(msgs);
                }

                frame1.frame_numbers.insert(record.frame_number);
                fns_to_repoint.push(record.frame_number);
                if let Some(ts) = record.timestamp {
                    frame1.timestamps.insert(ts);
                    tss_to_repoint.push(ts);
                }

                if let Some(existing_msgs) = frame1.digitisers.get(&record.digitiser_id)
                    && let Some(conflicting) = existing_msgs.first().cloned()
                {
                    new_dups.push(DuplicateEvent {
                        digitiser_id: record.digitiser_id,
                        incoming: record.clone(),
                        conflicting,
                        match_reason: MatchReason::FrameNumberAndTimestamp,
                    });
                }

                frame1
                    .digitisers
                    .entry(record.digitiser_id)
                    .or_default()
                    .push(record);
            }

            for fn_num in fns_to_repoint {
                self.frame_by_number.insert(fn_num, id1);
            }
            for ts in tss_to_repoint {
                self.frame_by_timestamp.insert(ts, id1);
            }
            for dup in new_dups {
                self.record_duplicate(dup.clone());
                duplicates.push(dup);
            }
        } else {
            self.handle_single_match(id1, record, MatchReason::FrameNumberOnly, duplicates);
        }
    }

    fn record_duplicate(&mut self, dup_event: DuplicateEvent) {
        self.stats.total_duplicates += 1;
        *self
            .stats
            .per_digitiser_duplicates
            .entry(dup_event.digitiser_id)
            .or_insert(0) += 1;
        if self.stats.duplicate_events.len() < MAX_DETAILED_EVENTS {
            self.stats.duplicate_events.push(dup_event);
        }
    }

    fn evict_if_needed(&mut self) {
        if self.max_frames == 0 {
            return;
        }

        while self.frames.len() > self.max_frames {
            if let Some(oldest_id) = self.insertion_order.pop_front() {
                if let Some(evicted_frame) = self.frames.remove(&oldest_id) {
                    for fn_num in evicted_frame.frame_numbers {
                        if self.frame_by_number.get(&fn_num) == Some(&oldest_id) {
                            self.frame_by_number.remove(&fn_num);
                        }
                    }
                    for ts in evicted_frame.timestamps {
                        if self.frame_by_timestamp.get(&ts) == Some(&oldest_id) {
                            self.frame_by_timestamp.remove(&ts);
                        }
                    }
                }
            } else {
                break;
            }
        }
    }

    pub fn print_report(&self, topic: &str) {
        print_summary_report(topic, &self.stats);
    }
}

pub(crate) async fn run(args: DuplicateDigitisersOpts) -> miette::Result<()> {
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

    let mut detector = DuplicateDetector::new(args.max_frames);

    let mut interval_timer = args
        .report_interval
        .map(|secs| tokio::time::interval(std::time::Duration::from_secs(secs)));

    info!(
        "Listening for digitiser messages on topic \"{}\" (group: \"{}\")... Press Ctrl-C to finish and view report.",
        args.common.topic, args.common.consumer_group
    );

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Interrupt received. Generating duplicate report...");
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
                detector.print_report(&args.common.topic);
            }
            msg_res = consumer.recv() => {
                match msg_res {
                    Err(e) => warn!("Kafka error: {e}"),
                    Ok(msg) => {
                        let partition = msg.partition();
                        let offset = msg.offset();
                        let key_str = format_kafka_key(msg.key());

                        if let Some(payload) = msg.payload()
                            && let Some((digitiser_id, frame_number, timestamp, message_type)) =
                                extract_message_info(payload)
                        {
                            let record = MessageRecord {
                                digitiser_id,
                                frame_number,
                                timestamp,
                                message_type,
                                partition,
                                offset,
                                key: key_str,
                                received_at: Utc::now(),
                            };
                            let duplicates = detector.process_message(record);
                            for dup in duplicates {
                                warn!(
                                    "Duplicate digitiser ID {} detected! Reason: {}. Conflicting message (partition: {}, offset: {}, frame_number: {}, timestamp: {:?}); Duplicate message (partition: {}, offset: {}, frame_number: {}, timestamp: {:?})",
                                    dup.digitiser_id,
                                    dup.match_reason,
                                    dup.conflicting.partition,
                                    dup.conflicting.offset,
                                    dup.conflicting.frame_number,
                                    dup.conflicting.timestamp.map(|t| t.to_string()),
                                    dup.incoming.partition,
                                    dup.incoming.offset,
                                    dup.incoming.frame_number,
                                    dup.incoming.timestamp.map(|t| t.to_string()),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    detector.print_report(&args.common.topic);
    Ok(())
}

fn format_kafka_key(key_bytes: Option<&[u8]>) -> String {
    match key_bytes {
        None => "<none>".to_string(),
        Some([]) => "<empty>".to_string(),
        Some(k) => String::from_utf8_lossy(k).into_owned(),
    }
}

pub fn extract_message_info(
    payload: &[u8],
) -> Option<(DigitizerId, u32, Option<MessageTimestamp>, &'static str)> {
    if digitizer_analog_trace_message_buffer_has_identifier(payload) {
        match root_as_digitizer_analog_trace_message(payload) {
            Ok(msg) => {
                let id = msg.digitizer_id();
                let meta = msg.metadata();
                let fn_num = meta.frame_number();
                let ts = meta.timestamp().map(MessageTimestamp::from_gps_time);
                Some((id, fn_num, ts, "dat2"))
            }
            Err(e) => {
                warn!("Failed to parse dat2 trace message: {e}");
                None
            }
        }
    } else if digitizer_event_list_message_buffer_has_identifier(payload) {
        match root_as_digitizer_event_list_message(payload) {
            Ok(msg) => {
                let id = msg.digitizer_id();
                let meta = msg.metadata();
                let fn_num = meta.frame_number();
                let ts = meta.timestamp().map(MessageTimestamp::from_gps_time);
                Some((id, fn_num, ts, "dev2"))
            }
            Err(e) => {
                warn!("Failed to parse dev2 event list message: {e}");
                None
            }
        }
    } else {
        debug!("Message payload does not match dat2 or dev2 flatbuffer identifier");
        None
    }
}

pub fn print_summary_report(topic: &str, stats: &DuplicateStats) {
    println!(
        "\n============================= Duplicate Digitisers Report ============================="
    );
    println!("Topic: {topic}");
    println!("Total messages processed: {}", stats.total_messages);
    println!(
        "Total duplicate instances detected: {}",
        stats.total_duplicates
    );

    if stats.total_duplicates == 0 {
        println!("No duplicate digitiser IDs observed in the same frame.");
        println!(
            "========================================================================================\n"
        );
        return;
    }

    println!(
        "\n{:<12} | {:<16} | {:<16}",
        "Digitiser ID", "Total Messages", "Duplicates Found"
    );
    println!("{:-<12}-+-{:-<16}-+-{:-<16}", "", "", "");

    for (id, total) in &stats.per_digitiser_messages {
        let dups = stats.per_digitiser_duplicates.get(id).copied().unwrap_or(0);
        println!("{:<12} | {:<16} | {:<16}", id, total, dups);
    }

    println!(
        "\n----------------------------- Duplicate Instances (First {}) ----------------------------",
        stats.duplicate_events.len()
    );
    for (idx, dup) in stats.duplicate_events.iter().enumerate() {
        let ts_str = match dup.incoming.timestamp {
            Some(ts) => ts.to_string(),
            None => "<none>".to_string(),
        };
        let prev_ts_str = match dup.conflicting.timestamp {
            Some(ts) => ts.to_string(),
            None => "<none>".to_string(),
        };

        println!(
            "{}. Digitiser ID: {}\n   Reason: {}\n   Incoming message:    type: {}, frame: {}, ts: {}, part: {}, offset: {}, key: \"{}\", rec: {}\n   Conflicting message: type: {}, frame: {}, ts: {}, part: {}, offset: {}, key: \"{}\", rec: {}",
            idx + 1,
            dup.digitiser_id,
            dup.match_reason,
            dup.incoming.message_type,
            dup.incoming.frame_number,
            ts_str,
            dup.incoming.partition,
            dup.incoming.offset,
            dup.incoming.key,
            dup.incoming.received_at.format("%Y-%m-%d %H:%M:%S UTC"),
            dup.conflicting.message_type,
            dup.conflicting.frame_number,
            prev_ts_str,
            dup.conflicting.partition,
            dup.conflicting.offset,
            dup.conflicting.key,
            dup.conflicting.received_at.format("%Y-%m-%d %H:%M:%S UTC"),
        );
    }

    if stats.total_duplicates as usize > stats.duplicate_events.len() {
        println!(
            "... and {} more duplicate instances (omitted from detail list).",
            stats.total_duplicates as usize - stats.duplicate_events.len()
        );
    }
    println!(
        "========================================================================================\n"
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use digital_muon_streaming_types::{
        dat2_digitizer_analog_trace_v2_generated::{
            DigitizerAnalogTraceMessage, DigitizerAnalogTraceMessageArgs,
            finish_digitizer_analog_trace_message_buffer,
        },
        dev2_digitizer_event_v2_generated::{
            DigitizerEventListMessage, DigitizerEventListMessageArgs,
            finish_digitizer_event_list_message_buffer,
        },
        flatbuffers::FlatBufferBuilder,
        frame_metadata_v2_generated::{FrameMetadataV2, FrameMetadataV2Args},
    };

    fn make_test_record(
        digitiser_id: DigitizerId,
        frame_number: u32,
        timestamp_opt: Option<DateTime<Utc>>,
        offset: i64,
    ) -> MessageRecord {
        MessageRecord {
            digitiser_id,
            frame_number,
            timestamp: timestamp_opt.map(MessageTimestamp::Utc),
            message_type: "dat2",
            partition: 0,
            offset,
            key: format!("{digitiser_id}"),
            received_at: Utc::now(),
        }
    }

    #[test]
    fn test_single_message_no_duplicates() {
        let mut detector = DuplicateDetector::new(100);
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap();
        let rec = make_test_record(1, 100, Some(t), 1);
        let dups = detector.process_message(rec);
        assert!(dups.is_empty());
        assert_eq!(detector.stats().total_messages, 1);
        assert_eq!(detector.stats().total_duplicates, 0);
    }

    #[test]
    fn test_different_digitisers_same_frame() {
        let mut detector = DuplicateDetector::new(100);
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap();
        let rec1 = make_test_record(1, 100, Some(t), 1);
        let rec2 = make_test_record(2, 100, Some(t), 2);
        assert!(detector.process_message(rec1).is_empty());
        assert!(detector.process_message(rec2).is_empty());
        assert_eq!(detector.stats().total_messages, 2);
        assert_eq!(detector.stats().total_duplicates, 0);
    }

    #[test]
    fn test_duplicate_same_frame_number_and_timestamp() {
        let mut detector = DuplicateDetector::new(100);
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap();
        let rec1 = make_test_record(1, 100, Some(t), 1);
        let rec2 = make_test_record(1, 100, Some(t), 2);
        assert!(detector.process_message(rec1).is_empty());
        let dups = detector.process_message(rec2);
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].digitiser_id, 1);
        assert_eq!(dups[0].match_reason, MatchReason::FrameNumberAndTimestamp);
        assert_eq!(detector.stats().total_duplicates, 1);
    }

    #[test]
    fn test_duplicate_same_frame_number_different_timestamp() {
        let mut detector = DuplicateDetector::new(100);
        let t1 = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 1).unwrap();
        let rec1 = make_test_record(1, 100, Some(t1), 1);
        let rec2 = make_test_record(1, 100, Some(t2), 2);
        assert!(detector.process_message(rec1).is_empty());
        let dups = detector.process_message(rec2);
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].digitiser_id, 1);
        assert_eq!(dups[0].match_reason, MatchReason::FrameNumberOnly);
        assert_eq!(detector.stats().total_duplicates, 1);
    }

    #[test]
    fn test_duplicate_different_frame_number_same_timestamp() {
        let mut detector = DuplicateDetector::new(100);
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap();
        let rec1 = make_test_record(1, 100, Some(t), 1);
        let rec2 = make_test_record(1, 101, Some(t), 2);
        assert!(detector.process_message(rec1).is_empty());
        let dups = detector.process_message(rec2);
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].digitiser_id, 1);
        assert_eq!(dups[0].match_reason, MatchReason::TimestampOnly);
        assert_eq!(detector.stats().total_duplicates, 1);
    }

    #[test]
    fn test_frame_merge_duplicate_detection() {
        let mut detector = DuplicateDetector::new(100);
        let t1 = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 5).unwrap();

        // Frame A: frame_number = 10, timestamp = t1, digitiser = 1
        let rec1 = make_test_record(1, 10, Some(t1), 1);
        assert!(detector.process_message(rec1).is_empty());

        // Frame B: frame_number = 20, timestamp = t2, digitiser = 1
        let rec2 = make_test_record(1, 20, Some(t2), 2);
        assert!(detector.process_message(rec2).is_empty());

        // Message 3: Digitiser 2 with frame_number = 10 AND timestamp = t2!
        // This bridges Frame A and Frame B into a single frame.
        // Both Frame A and Frame B already had digitiser 1!
        let rec3 = make_test_record(2, 10, Some(t2), 3);
        let dups = detector.process_message(rec3);
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].digitiser_id, 1);
        assert_eq!(dups[0].match_reason, MatchReason::FrameMerge);
        assert_eq!(detector.stats().total_duplicates, 1);
    }

    #[test]
    fn test_cache_eviction_limits_active_frames() {
        let mut detector = DuplicateDetector::new(2);
        let t1 = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 1).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 2).unwrap();
        let t3 = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 3).unwrap();

        let _ = detector.process_message(make_test_record(1, 1, Some(t1), 1));
        let _ = detector.process_message(make_test_record(1, 2, Some(t2), 2));
        assert_eq!(detector.active_frames_count(), 2);

        // Third frame causes eviction of frame 1
        let _ = detector.process_message(make_test_record(1, 3, Some(t3), 3));
        assert_eq!(detector.active_frames_count(), 2);
        assert_eq!(detector.stats().total_messages, 3);
    }

    #[test]
    fn test_flatbuffer_extraction_trace_and_event() {
        let mut fbb = FlatBufferBuilder::new();
        let gps_time = GpsTime::new(26, 282, 14, 30, 0, 0, 0, 0);

        let metadata_args = FrameMetadataV2Args {
            timestamp: Some(&gps_time),
            period_number: 1,
            protons_per_pulse: 10,
            running: true,
            frame_number: 777,
            veto_flags: 0,
        };
        let meta = FrameMetadataV2::create(&mut fbb, &metadata_args);

        let trace_msg = DigitizerAnalogTraceMessageArgs {
            digitizer_id: 42,
            metadata: Some(meta),
            sample_rate: 1000,
            channels: None,
        };
        let trace = DigitizerAnalogTraceMessage::create(&mut fbb, &trace_msg);
        finish_digitizer_analog_trace_message_buffer(&mut fbb, trace);

        let trace_bytes = fbb.finished_data().to_vec();
        let extracted_trace = extract_message_info(&trace_bytes);
        assert!(extracted_trace.is_some());
        let (id, fn_num, ts, msg_type) = extracted_trace.unwrap();
        assert_eq!(id, 42);
        assert_eq!(fn_num, 777);
        assert!(ts.is_some());
        assert_eq!(msg_type, "dat2");

        // Now test event message
        let mut fbb2 = FlatBufferBuilder::new();
        let meta2 = FrameMetadataV2::create(&mut fbb2, &metadata_args);
        let event_msg = DigitizerEventListMessageArgs {
            digitizer_id: 84,
            metadata: Some(meta2),
            time: None,
            voltage: None,
            channel: None,
        };
        let event = DigitizerEventListMessage::create(&mut fbb2, &event_msg);
        finish_digitizer_event_list_message_buffer(&mut fbb2, event);

        let event_bytes = fbb2.finished_data().to_vec();
        let extracted_event = extract_message_info(&event_bytes);
        assert!(extracted_event.is_some());
        let (id2, fn_num2, ts2, msg_type2) = extracted_event.unwrap();
        assert_eq!(id2, 84);
        assert_eq!(fn_num2, 777);
        assert!(ts2.is_some());
        assert_eq!(msg_type2, "dev2");
    }

    #[test]
    fn test_multiple_duplicates_same_digitiser() {
        let mut detector = DuplicateDetector::new(100);
        let t = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap();
        let rec1 = make_test_record(5, 50, Some(t), 1);
        let rec2 = make_test_record(5, 50, Some(t), 2);
        let rec3 = make_test_record(5, 50, Some(t), 3);

        assert!(detector.process_message(rec1).is_empty());
        let dups2 = detector.process_message(rec2);
        assert_eq!(dups2.len(), 1);
        let dups3 = detector.process_message(rec3);
        assert_eq!(dups3.len(), 1);

        assert_eq!(detector.stats().total_messages, 3);
        assert_eq!(detector.stats().total_duplicates, 2);
        assert_eq!(detector.stats().per_digitiser_duplicates.get(&5), Some(&2));
    }

    #[test]
    fn test_missing_timestamp_matches_by_frame_number() {
        let mut detector = DuplicateDetector::new(100);
        let rec1 = make_test_record(3, 10, None, 1);
        let rec2 = make_test_record(3, 10, None, 2);
        let rec3 = make_test_record(3, 11, None, 3);

        assert!(detector.process_message(rec1).is_empty());
        let dups = detector.process_message(rec2);
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].match_reason, MatchReason::FrameNumberOnly);

        // rec3 has different frame number and no timestamp -> not in same frame
        assert!(detector.process_message(rec3).is_empty());
    }

    #[test]
    fn test_unlimited_cache_max_frames_zero() {
        let mut detector = DuplicateDetector::new(0);
        for i in 0..50 {
            let t = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, i).unwrap();
            let _ = detector.process_message(make_test_record(1, i, Some(t), i as i64));
        }
        assert_eq!(detector.active_frames_count(), 50);
        assert_eq!(detector.stats().total_messages, 50);
    }

    #[test]
    fn test_print_summary_report_does_not_panic() {
        let mut detector = DuplicateDetector::new(10);
        detector.print_report("test-topic");

        let t = Utc.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap();
        let _ = detector.process_message(make_test_record(1, 100, Some(t), 1));
        let _ = detector.process_message(make_test_record(1, 100, Some(t), 2));
        detector.print_report("test-topic");
    }
}
