//! Appears at the top of each page.
use crate::app::TopLevelContext;
use leptos::{IntoView, component, prelude::*, view};

/// Control and title bar, located above [Main].
#[component]
pub(crate) fn TopBar() -> impl IntoView {
    let client_side_data = use_context::<TopLevelContext>()
        .expect("TopLevelContext should be provided, this should never fail.")
        .client_side_data;

    let red_panda_link = client_side_data
        .link_to_redpanda_console
        .map(|link| view! {<a href = {link.clone()}><span>Redpanda Console</span></a>});
    let broker_name = client_side_data.broker_name;

    let git_revision = option_env!("GIT_REVISION").unwrap_or("Git Rev Unknown");
    let issue_url = format!(
        "https://github.com/STFC-ICD-Research-and-Design/supermusr-data-pipeline/issues/new?title=Trace Viewer ({git_revision}): &template=bug-report.md"
    );
    let feature_url = format!(
        "https://github.com/STFC-ICD-Research-and-Design/supermusr-data-pipeline/issues/new?title=Trace Viewer ({git_revision}): &template=feature.md"
    );
    let home_url = client_side_data.public_url.to_string();
    let help_url = format!("{}/help", client_side_data.public_url.clone());

    view! {
        <div class = "topbar">
            <div class = "title-box">
                <div class = "title">"Trace Viewer"</div>
                <div class = "subtitle">{broker_name}</div>
            </div>
            <div class = "menu">
                <a href = {home_url}><span>Home</span></a>
                {red_panda_link}
                <a href = {issue_url}><span>"Report Issue"</span></a>
                <a href = {feature_url}><span>"Request Feature"</span></a>
                <a href = {help_url}><span>Help</span></a>
            </div>
        </div>
    }
}
