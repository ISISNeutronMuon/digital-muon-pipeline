//! Defines structs used throughout.
//!
//! These fall into two categories:
//! - Server-side only: these are gated behind the "ssr" feature flag.
//! - Client-Server transferable: these must implement [Clone], [Debug], [Serialize] and [Deserialize].
mod broker_info;
mod digitiser_messages;
mod search;
mod trace_messages;

use crate::{Channel, DigitizerId, Timestamp};
use cfg_if::cfg_if;
use serde::{Deserialize, Serialize};

pub use broker_info::{BrokerInfo, BrokerTopicInfo};
pub use search::{SearchTarget, SearchTargetBy, SearchTargetMode};
pub use trace_messages::{SearchSummary, SelectedTraceIndex, TracePlotly, TraceSummary};
use url::Url;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        mod server_only;

        use clap::Args; // This should be imported only for server-side use.

        pub(crate) use digitiser_messages::{DigitiserMetadata, DigitiserTrace, EventList, Trace};
        pub(crate) use server_only::{Cache, BorrowedMessageError, SearchResults, EventListMessage, FBMessage, TraceMessage};

        pub use server_only::ServerSideData;
    }
}

/// Contains the names of the Kafka topics as set in the command line.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(Args))]
pub struct Topics {
    /// Kafka trace topic.
    #[cfg_attr(feature = "ssr", clap(long))]
    pub trace_topic: String,

    /// Kafka digitiser event list topic.
    #[cfg_attr(feature = "ssr", clap(long))]
    pub digitiser_event_topic: Vec<String>,
}

/// Contains the settings defined in the CLI used as default values in the UI's inputs.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(Args))]
pub struct DefaultData {
    /// The timestamp of the frame to search for, should be in the format "YYYY-MM-DD hh:mm:ss.f <timezone>".
    #[cfg_attr(feature = "ssr", clap(long))]
    pub(crate) timestamp: Option<Timestamp>,

    /// The digitiser Ids to search for.
    #[cfg_attr(feature = "ssr", clap(long))]
    pub(crate) digitiser_ids: Option<Vec<DigitizerId>>,

    /// The channels to search for.
    #[cfg_attr(feature = "ssr", clap(long))]
    pub(crate) channels: Option<Vec<Channel>>,

    /// The maximum number of messages to collect.
    #[cfg_attr(feature = "ssr", clap(long))]
    pub(crate) number: Option<usize>,

    /// Default Kafka timeout for polling the broker for topic info.
    /// If this feature is failing, then increasing this value may help.
    #[cfg_attr(feature = "ssr", clap(long, default_value = "1000"))]
    pub(crate) poll_broker_timeout_ms: u64,
}

/// Encapsulates all run-time settings which are available to the client.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientSideData {
    pub default_data: DefaultData,
    pub broker_name: String,
    pub link_to_redpanda_console: Option<String>,
    pub refresh_session_interval_sec: u64,
    pub public_url: Url,
}
