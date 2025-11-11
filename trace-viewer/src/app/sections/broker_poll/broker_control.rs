use crate::app::{TopLevelContext, server_functions::PollBroker};
use leptos::{IntoView, component, html::Input, prelude::*, view};

#[component]
pub fn BrokerPoller(poll_broker_action: ServerAction<PollBroker>) -> impl IntoView {
    let client_side_data = use_context::<TopLevelContext>()
        .expect("TopLevelContext should be provided, this should never fail.")
        .client_side_data;
    let default_data = client_side_data.default_data;
    let eventlist_topics = client_side_data.eventlist_topics;

    /*let events_topic_index = use_context::<MainLevelContext>()
        .expect("MainLevelContext should be provided, this should never fail.")
        .events_topic_index;*/

    let timeout_ms_ref = NodeRef::<Input>::new();

    let poll_broker_timeout_ms = default_data.poll_broker_timeout_ms;

    view! {
        <ActionForm action = poll_broker_action>
            <div class = "broker-poll">
                <label class = "panel-item" for = "events_topic_index">
                    "Event List Topic:"
                    <IndexedSelectList name = "events_topic_index".into() id = "events_topic_index".into() items = eventlist_topics/>
                </label>
                <label class = "panel-item" for = "poll_broker_timeout_ms">
                    "Poll Broker Timeout (ms):"
                    <input class = "small" name = "poll_broker_timeout_ms" id = "poll_broker_timeout_ms" value = poll_broker_timeout_ms type = "number" node_ref = timeout_ms_ref />
                </label>
                <input type = "submit" value = "Poll Broker"/>
            </div>
        </ActionForm>
    }
}

#[component]
fn IndexedSelectList(
    name: String,
    id: String,
    items: Vec<String>
) -> impl IntoView {
    let items = items.into_iter().enumerate().collect::<Vec<_>>();
    view! {
        <select name = name id = id
            /*on:change = move |ev| signal.set(
                event_target_value(&ev)
                    .parse()
                    .expect("Int value should parse, this should never fail.")
            )*/
        >
            <For each = move || items.clone() key = |(idx,_)|*idx let ((idx, item))>
                <option /*selected={signal.get() == idx}*/  value = idx>
                    {item}
                </option>
            </For>
        </select>
    }
}
