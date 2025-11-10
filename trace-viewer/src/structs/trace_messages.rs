use crate::structs::SearchTarget;
use serde::{Deserialize, Serialize};

/// Encapsulates the data needed to summarise the results of a search in the results section.
#[derive(Clone, Serialize, Deserialize)]
pub struct SearchSummary {
    /// Topic from which the events were captured.
    pub events_topic: String,
    pub target: SearchTarget,
    pub traces: Vec<TraceSummary>,
}

/// Encapsulates the data needed to summarise a message in the results list.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceSummary {
    /// Date of the message.
    pub date: String,
    /// Time of the message.
    pub time: String,
    /// Frame Number of the message.
    pub frame_number: u32,
    /// Period Number of the message.
    pub period_number: u64,
    /// Protons Per Pulse value of the message.
    pub protons_per_pulse: u8,
    /// Running flag of the message.
    pub running: bool,
    /// Veto Flags of the message.
    pub veto_flags: u16,
    /// Digitiser Id of the message.
    pub id: u8,
    /// List of channels in the message.
    pub channels: Vec<u32>,
    /// Index of the message in the corresponding [Cache].
    pub index: usize,
}

/// Represents a trace message and channel stored in a [Cache].
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SelectedTraceIndex {
    /// The index of the trace message in the corresponding [Cache].
    pub(crate) index: usize,
    /// The channel of the trace message indexed by [index] in the corresponding [Cache].
    pub(crate) channel: u32,
}

/// Encapsulates data needed by the [DisplayGraph] component.
/// Should be created by [create_plotly()]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TracePlotly {
    /// Text to be displayed as a graph heading.
    pub title: String,
    /// Json string of the trace data plotly graph.
    pub trace_data: String,
    /// If present, Json string of the event list data plotly graph.
    pub eventlist_data: Option<String>,
    /// Json string of the plotly layout to use.
    pub layout: String,
}
