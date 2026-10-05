//! A closable box, stacked vertically, with a header and content space.
use crate::app::components::toggle_closed;
use leptos::{IntoView, component, prelude::*, view};

/// Closable top-level container which should only contain functionally self-contained components,
/// i.e. components within each `Section` should not depend on components inside another `Section`,
/// although multiple sections can depend on items from a shared context object.
///
/// # Example
/// A section is a structure composed of nested div elements as follows:
/// ```rust
/// view!{
///     <div class = "section closable-container">
///         <div class = "name closable-control" on:click:target = move |..| ..>
///             ...
///         </div>
///         <div id = {...} class = "content closable">
///             {children()}
///         </div>
///     </div>
/// }
/// ```
///
/// # Parameters
/// - id: id field of the "content"-classed div.
/// - text: text to appear in the "name"-classed div.
/// - children: children appearing in the "content"-classed div of the section.
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
