use crate::{
    app::sections::results::{
        context::ResultsLevelContext, search_results::SelectTraceLevelContext,
    },
    structs::SearchTargetBy,
};
use leptos::{IntoView, component, either::Either, prelude::*, view};

/// Panel containing settings which control how results of a search are shown.
#[component]
pub(crate) fn ResultsSettingsPanel() -> impl IntoView {
    let target = use_context::<SelectTraceLevelContext>()
        .expect("SelectTraceLevelContext should be provided, this should never fail.")
        .target;

    view! {
        <div class = "search-results-settings">
            <ShowSelectedChannelsOnly by = target.by />
        </div>
    }
}

/// Checkbox allowing the user to show only those channels specified in the selection criteria,
/// or all channels in the digitiser messages. This will only display if the search mode is
/// set to `SearchTargetBy::ByChannels`.
///
/// # Parameters
/// - by: the current search mode.
#[component]
pub(crate) fn ShowSelectedChannelsOnly(by: SearchTargetBy) -> impl IntoView {
    let result_level_context = use_context::<ResultsLevelContext>()
        .expect("ResultsLevelContext should be provided, this should never fail.");

    match by {
        SearchTargetBy::ByChannels { channels: _ } => Either::Left(view! {
            <label class = "results-settings-input" for = "selected-channels-only">
                "Selected channels only:"
                <input class = "results-settings-input" name = "selected-channels-only" id = "selected-channels-only" type = "checkbox"
                    bind:value = result_level_context.selected_channels_only
                />
            </label>
        }),
        _ => Either::Right(()),
    }
}
