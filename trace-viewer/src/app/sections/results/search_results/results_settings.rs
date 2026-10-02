use crate::{
    app::sections::results::{
        context::ResultsLevelContext, search_results::SelectTraceLevelContext,
    },
    structs::SearchTargetBy,
};
use leptos::{IntoView, component, either::Either, prelude::*, view};

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
