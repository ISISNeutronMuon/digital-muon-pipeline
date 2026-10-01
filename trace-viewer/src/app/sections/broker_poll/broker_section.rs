use crate::app::{
    components::Section,
    sections::broker_poll::{broker_control::BrokerPoller, broker_info::DisplayBrokerInfo},
    server_functions::PollBroker,
};
use leptos::{IntoView, component, prelude::*, view};

/// Top-level section component containing the `Section` which allows the user to run a poll broker request and display the results.
#[component]
pub(crate) fn BrokerSection() -> impl IntoView {
    let poll_broker_action = ServerAction::<PollBroker>::new();
    view! {
        <Section text = "Broker" id = "broker">
            <BrokerPoller poll_broker_action/>
            <DisplayBrokerInfo poll_broker_action />
        </Section>
    }
}
