//! A closable box, stacked vertically, with a header and content space.
use crate::app::components::toggle_closed;
use leptos::{IntoView, component, prelude::*, view};

/// Closable top-level container which should only contain functionally self-contained components,
/// i.e. components within each `Section` should not depend on components inside another `Section`.
#[component]
pub(crate) fn Section(id: &'static str, text: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class = "section closable-container">
            <div class = "name closable-control" on:click:target = move |e| toggle_closed(e.target().parent_element())>
                {text}
            </div>
            <div id = {id} class = "content closable">
                {children()}
            </div>
        </div>
    }
}
