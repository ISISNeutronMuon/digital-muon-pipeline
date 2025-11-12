mod digitiser_message;
mod results_settings;
mod select_channel;

use crate::{
    app::{
        TopLevelContext,
        sections::results::search_results::{
            digitiser_message::DigitiserMessage, results_settings::ResultsSettingsPanel,
        },
    },
    structs::{
        SearchSummary, SearchTarget, SearchTargetBy, SearchTargetMode, SelectedTraceIndex,
        TraceSummary,
    },
};
use leptos::{IntoView, component, either::Either, prelude::*, view};
use std::collections::BTreeMap;

type TraceSummariesByTime = Vec<(String, Vec<TraceSummary>)>;

fn sort_trace_summaries(trace_summaries: Vec<TraceSummary>) -> Vec<(String, TraceSummariesByTime)> {
    let mut trace_by_date_and_time = BTreeMap::<String, BTreeMap<String, Vec<TraceSummary>>>::new();

    for trace_summary in trace_summaries.into_iter() {
        trace_by_date_and_time
            .entry(trace_summary.date.clone())
            .or_default()
            .entry(trace_summary.time.clone())
            .or_default()
            .push(trace_summary);
    }

    trace_by_date_and_time
        .into_iter()
        .map(|(date, by_time)| (date, by_time.into_iter().collect::<Vec<_>>()))
        .collect::<Vec<_>>()
}

/// This struct enable a degree of type-checking for the [use_context]/[use_context] functions.
/// Any component making use of the following fields should call `use_context::<SelectTraceLevelContext>()`
/// and select the desired field.
#[derive(Clone)]
struct SelectTraceLevelContext {
    eventlist_topic_indices: Vec<usize>,
    target: SearchTarget,
    num_results: usize,
    select_trace_index: RwSignal<Option<SelectedTraceIndex>>,
}

#[component]
pub(crate) fn SearchResultsPanel(search_summary: SearchSummary) -> impl IntoView {
    provide_context(SelectTraceLevelContext {
        eventlist_topic_indices: search_summary.eventlist_topic_indices,
        target: search_summary.target,
        num_results: search_summary.traces.len(),
        select_trace_index: RwSignal::<Option<SelectedTraceIndex>>::new(None),
    });

    let trace_by_date_and_time = sort_trace_summaries(search_summary.traces);

    view! {
        <div class = "content search-results" id = "search-results">
            <SearchSummary />
            <ResultsSettingsPanel />
            <For
                each = move ||trace_by_date_and_time.clone().into_iter()
                key = |(date,_)|date.clone()
                let((date, trace_summaries_by_time))>
                    <SearchResultsByDate date trace_summaries_by_time/>
            </For>
        </div>
    }
}

#[component]
pub(crate) fn SearchSummary() -> impl IntoView {
    let eventlist_topics = use_context::<TopLevelContext>()
        .expect("")
        .client_side_data
        .eventlist_topics;
    let SelectTraceLevelContext {
        eventlist_topic_indices,
        target,
        num_results,
        select_trace_index: _,
    } = use_context::<SelectTraceLevelContext>().expect("");

    let eventlist_topic_indices = eventlist_topic_indices
        .into_iter()
        .map(|idx| eventlist_topics.get(idx).expect("").clone())
        .collect::<Vec<_>>();

    view! {
        <div class = "search-results-summary">
            "Found " {num_results} " results matching search criteria:"
            <ul>
                <li>
                    "Events capured from topic:"
                    <ul>
                        <For each=move||eventlist_topic_indices.clone()
                            key=move|topic|topic.clone()
                            children=|topic|view!{<li> {topic} </li>}
                        />
                    </ul>
                </li>
                {match target.mode {
                    SearchTargetMode::Timestamp { timestamp } => view! {
                        <li> {format!("At or after: {} {}", timestamp.date_naive(), timestamp.time())} </li>
                    },
                    SearchTargetMode::Dragnet {timestamp, backstep, forward_distance } => view! {
                        <li> {format!(
                            "Around: {} {}, message range: [{backstep}, {forward_distance}]",
                            timestamp.date_naive(), timestamp.time())
                        } </li>
                    }
                }}
                {match target.by {
                    SearchTargetBy::All => Either::Left(()),
                    SearchTargetBy::ByChannels { channels } => Either::Right(view!{
                        <li> {format!("Containing at least one channel of: {{ {} }}", channels.iter().map(ToString::to_string).collect::<Vec<_>>().join(","))} </li>
                    }),
                    SearchTargetBy::ByDigitiserIds { digitiser_ids } => Either::Right(view!{
                        <li> {format!("With Digitiser Id in: {{ {} }}", digitiser_ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(","))} </li>
                    }),
                }}
                <li> "Maximum results: " {target.number} </li>
            </ul>
        </div>
    }
}

#[component]
fn SearchResultsByDate(
    date: String,
    trace_summaries_by_time: Vec<(String, Vec<TraceSummary>)>,
) -> impl IntoView {
    view! {
        <div class = "search-results-by-date">
            <div class = "search-results-date"> "Date: " {date} </div>
            <For
                each = move ||trace_summaries_by_time.clone().into_iter()
                key = |(time,_)|time.clone()
                let((time, trace_summaries))
            >
                <SearchResultsByTime time trace_summaries />
            </For>
        </div>
    }
}

#[component]
fn SearchResultsByTime(time: String, mut trace_summaries: Vec<TraceSummary>) -> impl IntoView {
    trace_summaries.sort_by(|summary1, summary2| {
        summary1
            .id
            .partial_cmp(&summary2.id)
            .expect("Ordering should complete, this should never fail.")
    });
    view! {
        <div class = "search-results-by-time">
            <div class = "search-results-time"> "Time: " {time} </div>
            <For
                each = move ||trace_summaries.clone().into_iter()
                key = ToOwned::to_owned
                let(trace_summary)
            >
                <DigitiserMessage trace_summary />
            </For>
        </div>
    }
}
