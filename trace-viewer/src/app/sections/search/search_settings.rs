use crate::app::{sections::search::context::SearchLevelContext, TopLevelContext};
use leptos::{IntoView, component, either::EitherOf3, prelude::*, view};
use std::{collections::BTreeSet, str::FromStr};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};


#[component]
pub(crate) fn CheckBoxList(names: Vec<(usize,String)>, index_set: RwSignal<BTreeSet<usize>>) -> impl IntoView {
    view! {
        <fieldset name = "eventlist-sources" >
            <For each = move || names.clone()
                key = |(idx,_)|*idx
                let ((idx, item))
                >
                    <label for = {item.clone()}>
                        {item.clone()}
                        <input name = {item.clone()} id = {item.clone()} type = "checkbox"
                            value = {move ||index_set.get().contains(&idx)}
                            on:change = move |ev| {
                                let mut index_set_inner = index_set.read();
                                if event_target_checked(&ev) {
                                    index_set_inner.insert(idx);
                                } else {
                                    index_set_inner.remove(&idx);
                                }
                                index_set.set(index_set_inner);
                            }
                        />
                    </label>
                </For>
        </fieldset>
    }
}

#[component]
pub(crate) fn SearchSettings() -> impl IntoView {
    let search_level_context = use_context::<SearchLevelContext>()
        .expect("search_broker_node_refs should be provided, this should never fail.");

    let eventlist_topics = use_context::<TopLevelContext>()
        .expect("TopLevelContext should be provided, this should never fail.")
        .client_side_data
        .eventlist_topics
        .clone();
    
    let eventlist_topics = eventlist_topics.into_iter().enumerate().collect::<Vec<_>>();
    view! {
        <label for = "eventlist-sources">
            "Eventlist source topics:"
            <CheckBoxList names = {eventlist_topics.clone()} index_set = {search_level_context.eventlist_sources} />
        </label>
        <SearchMode />
        <label for = "date">
            "Date:"
            <input name = "date" id = "date" type = "date"
                value = {move ||search_level_context.date.get().to_string()}
                on:change = {move |ev|search_level_context.date.set(event_target_value(&ev).parse().expect("Date should parse, this should never fail."))}
            />
        </label>
        <label for = "time">
            "Time:"
            <input name = "time" id = "time" type = "text"
                value = {move ||search_level_context.time.get().to_string()}
                on:change = {move |ev|search_level_context.time.set(event_target_value(&ev).parse().expect("Time should parse, this should never fail."))}
            />
        </label>
        <label for = "number">
            "Number:"
            <input name = "number" id = "number" type = "text"
                value = {move ||search_level_context.number.get().to_string()}
                on:change = {move |ev|search_level_context.number.set(event_target_value(&ev).parse().expect("Number should parse, this should never fail."))}
            />
        </label>
        <Show when = move|| matches!(search_level_context.search_mode.get(), SearchMode::Dragnet)>
            <label for = "backstep">
                "Backstep:"
                <input name = "backstep" id = "backstep" type = "text"
                    value = {move ||search_level_context.backstep.get().to_string()}
                    on:change = {move |ev|search_level_context.backstep.set(event_target_value(&ev).parse().expect("Backstep should parse, this should never fail."))}
                />
            </label>
            <label for = "forward-dist">
                "Forward Distance:"
                <input name = "forward-dist" id = "forward-dist" type = "text"
                    value = {move ||search_level_context.forward_distance.get().to_string()}
                    on:change = {move |ev|search_level_context.forward_distance.set(event_target_value(&ev).parse().expect("Forward Distance should parse, this should never fail."))}
                />
            </label>
        </Show>

        <MatchCriteria />
        <MatchBy />
    }
}

#[derive(Default, Clone, EnumString, Display, EnumIter, PartialEq, Eq, Hash, Copy)]
pub(crate) enum SearchMode {
    #[default]
    #[strum(to_string = "From Timestamp")]
    Timestamp,
    #[strum(to_string = "Dragnet Search")]
    Dragnet,
}

#[component]
pub(crate) fn SearchMode() -> impl IntoView {
    let search_level_context = use_context::<SearchLevelContext>()
        .expect("search_broker_node_refs should be provided, this should never fail.");

    let search_mode = search_level_context.search_mode;

    view! {
        <label class = "panel-item" for = "search-mode">
            "Search Mode: "
            <select name = "search-mode" id = "search-mode" class = "panel-item"
                on:change = move |ev|
                    search_mode.set(
                        event_target_value(&ev)
                            .parse()
                            .expect("SearchMode value should parse, this should never fail.")
                        )>
                <For each = SearchMode::iter
                    key = ToOwned::to_owned
                    let(mode)
                >
                    <option selected={search_mode.get() == mode} value = {mode.to_string()}> {mode.to_string()} </option>
                </For>
            </select>
        </label>
    }
}

#[derive(Default, Clone, EnumString, Display, EnumIter, PartialEq, Eq, Hash, Copy)]
pub(crate) enum SearchBy {
    #[default]
    #[strum(to_string = "Match All")]
    All,
    #[strum(to_string = "By Channels")]
    ByChannels,
    #[strum(to_string = "By Digitiser Ids")]
    ByDigitiserIds,
}

#[component]
pub(crate) fn MatchCriteria() -> impl IntoView {
    let search_level_context = use_context::<SearchLevelContext>()
        .expect("search_broker_node_refs should be provided, this should never fail.");

    let search_by = search_level_context.search_by;

    view! {
        <label for = "match-criteria">
            "Match Criteria: "
            <select name = "match-criteria" id = "match-criteria" class = "panel-item"
                data-tooltip = "Choose which criteria to match on: by channel's contained, or by digitiser id."
                on:change = move |ev| search_by.set(
                    event_target_value(&ev)
                        .parse()
                        .expect("SearchBy value should parse, this should never fail.")
                )
            >
                <For
                    each = SearchBy::iter
                    key = ToOwned::to_owned
                    let(mode)
                >
                    <option selected={search_by.get() == mode}  value = {mode.to_string()}>{mode.to_string()}</option>
                </For>
            </select>
        </label>
    }
}

#[component]
pub(crate) fn MatchBy() -> impl IntoView {
    let search_level_context = use_context::<SearchLevelContext>()
        .expect("search_broker_node_refs should be provided, this should never fail.");

    fn parse_to_list<T: ToString>(list: &[T]) -> String {
        list.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }

    fn parse_from_list<T: FromStr>(str: String) -> Vec<T>
    where
        <T as FromStr>::Err: std::fmt::Debug,
    {
        str.split(",")
            .map(|x| x.parse())
            .collect::<Result<Vec<_>, _>>()
            .expect("This should do some validation checking. TODO")
    }

    move || match search_level_context.search_by.get() {
        SearchBy::All => EitherOf3::A(()),
        SearchBy::ByChannels => EitherOf3::B(view! {
            <label for = "channels">
                "Channels:"
                <input class = "panel-item" type = "text" id = "channels"
                    value = move ||parse_to_list(&search_level_context.channels.get())
                    on:change = move |ev|search_level_context.channels.set(parse_from_list(event_target_value(&ev).parse().expect("msg")))
                />
            </label>
        }),
        SearchBy::ByDigitiserIds => EitherOf3::C(view! {
            <label for = "digitiser-ids">
                "Digitiser IDs:"
                <input class = "panel-item" type = "text" id = "digitiser-ids"
                    value = move ||parse_to_list(&search_level_context.digitiser_ids.get())
                    on:change = move |ev|search_level_context.digitiser_ids.set(parse_from_list(event_target_value(&ev).parse().expect("msg")))
                />
            </label>
        }),
    }
}
