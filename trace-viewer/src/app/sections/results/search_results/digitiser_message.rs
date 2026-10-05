use crate::{
    app::{
        components::toggle_closed,
        sections::results::search_results::{
            SelectTraceLevelContext, select_channel::SelectChannels,
        },
    },
    structs::TraceSummary,
};
use leptos::{IntoView, component, prelude::*, view};

/// Displays the summary of a particular digitiser message, and allows the user to select a channel to display.
///
/// # Parameters
/// - trace_summary: summary of a particular digitiser message result.
#[component]
pub(super) fn DigitiserMessage(trace_summary: TraceSummary) -> impl IntoView {
    let selected_trace_index = use_context::<SelectTraceLevelContext>()
        .expect("SelectTraceLevelContext should be provided, this should never fail.")
        .select_trace_index;

    let selected_pred = move || {
        selected_trace_index
            .get()
            .is_some_and(|index_and_channel| index_and_channel.index == trace_summary.index)
    };

    let trace_summary_metadata = trace_summary.clone();

    view! {
        <div class = "digitiser-message" class = ("selected", selected_pred)>
            <div class = "digitiser-message-id"> "Id: " {trace_summary.id}</div>
            <SelectChannels
                index = trace_summary.index
                channels = trace_summary.channels
            />
            <Metadata trace_summary = trace_summary_metadata />
        </div>
    }
}

/// Displays the metadata of a particular digitiser message.
///
/// # Parameters
/// - trace_summary: summary of a particular digitiser message result.
#[component]
fn Metadata(trace_summary: TraceSummary) -> impl IntoView {
    view! {
        <div class = "digitiser-message-metadata closable-container closed">
            <div class = "digitiser-message-metadata-title closable-control"
                    on:click:target = move |e| toggle_closed(e.target().parent_element())>
                "Metadata"
            </div>
            <div class = "digitiser-message-metadata-content closable">
              <div> "Frame Number: "      {trace_summary.frame_number} </div>
              <div> "Period Number: "     {trace_summary.period_number} </div>
              <div> "Protons per Pulse: " {trace_summary.protons_per_pulse} </div>
              <div> "Running: "           {trace_summary.running} </div>
              <div> "VetoFlags: "         {trace_summary.veto_flags} </div>
            </div>
        </div>
    }
}
