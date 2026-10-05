use crate::{Channel, DigitizerId, Timestamp};
use serde::{Deserialize, Serialize};

/// Encapsulates the settings the client wishes to use in a search.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchTarget {
    pub mode: SearchTargetMode,
    pub by: SearchTargetBy,
    pub number: usize,
}

/// Which type of search to perform.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SearchTargetMode {
    /// Search for messages at or after a particular timestamp.
    Timestamp { timestamp: Timestamp },
    /// Collect messages within a particular window about the given timestamp.
    Dragnet {
        /// Timestamp to search for initially.
        timestamp: Timestamp,
        /// Number of messages behind the timestamped-message to search from.
        backstep: i64,
        /// Number of messages ahead to search.
        forward_distance: usize,
    },
}

/// Criteria to match messages on.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SearchTargetBy {
    /// Matches all messages.
    All,
    /// Only matches messages who contain one of the following channels.
    ByChannels { channels: Vec<Channel> },
    /// Only matches messages whose digitiser id is one of the following.
    ByDigitiserIds { digitiser_ids: Vec<DigitizerId> },
}
