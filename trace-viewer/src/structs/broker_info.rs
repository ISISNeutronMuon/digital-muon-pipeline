//! Structs which encapsulate the state of the broker's trace and eventlist topics.
//! 
//! In particular, these capture the offsets and status packet timestamps of
//! the first and last message on each topic.
use crate::Timestamp;
use serde::{Deserialize, Serialize};

/// Encapsulates the state of a topic in the broker.
/// 
/// Currently operates on partition 0 only
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrokerTopicInfo {
    /// Pair representing the low and high watermark of the topic (on partion 0).
    pub offsets: (i64, i64),
    /// Optional pair representing status packet timestamp of the first and last digitiser message on the topic.
    pub timestamps: Option<(Timestamp, Timestamp)>,
}

/// Encapsulates the state of the broker at a particular time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrokerInfo {
    /// Timestamp at which the state was captured.
    pub timestamp: Timestamp,
    /// State of the Trace topic.
    pub trace: BrokerTopicInfo,
    /// State of the Eventlist topic.
    pub events: BrokerTopicInfo,
}
