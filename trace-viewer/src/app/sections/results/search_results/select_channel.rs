use crate::{
    app::{
        main_content::MainLevelContext,
        sections::results::{
            context::ResultsLevelContext, search_results::SelectTraceLevelContext,
        },
        server_functions::CreateAndFetchPlotly,
    },
    structs::{SearchTargetBy, SelectedTraceIndex},
};
use leptos::{IntoView, component, ev::MouseEvent, prelude::*, view};

/// Allows user to select a channel in a trace message, whose trace and eventlist to display.
///
/// # Parameters
/// - index: the index in the stored traces this panel refers to.
/// - channels: list of channels that can be selected from.
#[component]
pub(super) fn SelectChannels(index: usize, mut channels: Vec<u32>) -> impl IntoView {
    let result_level_context = use_context::<ResultsLevelContext>()
        .expect("result_level_context should be provided, this should never fail.");
    let selected_channels_only = result_level_context.selected_channels_only.read_only();

    let select_trace_level_context = use_context::<SelectTraceLevelContext>()
        .expect("select_trace_level_context should be provided, this should never fail.");

    let channels_to_display = match select_trace_level_context.target.by {
        SearchTargetBy::ByChannels { channels } => Some(channels),
        _ => None,
    };

    channels.sort();
    view! {
        <div class = "channel-list">
            <For each = move ||channels.clone().into_iter()
                key = ToOwned::to_owned
                children = move |channel| {
                    let channels_to_display = channels_to_display.clone();
                    let show_channel = channels_to_display.as_ref()
                        .is_some_and(|channels_to_display|channels_to_display.contains(&channel));
                    move || (!selected_channels_only.get() || show_channel).then(||
                        view! { <Channel this_index_and_channel = SelectedTraceIndex { index, channel } /> }
                    )
                }
            />
        </div>
    }
}

/// Button which selects a particular channel in a trace message.
///
/// # Parameters
/// - this_index_and_channel: the index in the stored traces, and the channel this button refers to.
#[component]
fn Channel(this_index_and_channel: SelectedTraceIndex) -> impl IntoView {
    let main_context = use_context::<MainLevelContext>()
        .expect("MainLevelContext should be provided, this should never fail.");

    let create_and_fetch_plotly = use_context::<ResultsLevelContext>()
        .expect("ResultsLevelContext should be provided, this should never fail.")
        .create_and_fetch_plotly;

    let selected_trace_index = use_context::<SelectTraceLevelContext>()
        .expect("SelectTraceLevelContext should be provided, this should never fail.")
        .select_trace_index;

    let uuid = main_context.uuid;

    let SelectedTraceIndex { index, channel } = this_index_and_channel.clone();

    let on_click = {
        let this_index_and_channel = this_index_and_channel.clone();
        move |_: MouseEvent| {
            if let Some(uuid) = uuid.get() {
                selected_trace_index.set(Some(this_index_and_channel.clone()));
                create_and_fetch_plotly.dispatch(CreateAndFetchPlotly {
                    uuid,
                    index_and_channel: this_index_and_channel.clone(),
                });
            }
        }
    };

    let selected_pred = move || {
        selected_trace_index.get().is_some_and(|index_and_channel| {
            index_and_channel.index == index && index_and_channel.channel == channel
        })
    };

    view! {
        <div class = "channel"
            class = ("selected", selected_pred)
            on:click = on_click
        >
            {this_index_and_channel.channel}
        </div>
    }
}
