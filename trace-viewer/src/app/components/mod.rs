//! Defines Leptos components which are used throughout the rest of the [app] module.
mod display_errors;
mod section;

use leptos::{logging, tachys::renderer::dom::Element};

pub(crate) use display_errors::DisplayErrors;
pub(crate) use section::Section;

/// Toggles a closable `div` structure, that is turns a `div` into a button which hides or shows another `div`.
/// It does this by toggling the class `closed`, in the parent `div` (which should be passed as `element`).
///
/// # Parameters
/// - element: the parent `div`, containing both the button `div`, and the `div` to hide/show.
///
/// # Example
/// `toggle_closed` is used in the following way.
/// ```rust
/// view!{
///     <div class = "closable-container">
///         <div class = "closable-control" on:click:target = move |e| toggle_closed(e.target().parent_element())>
///             Click to Toggle
///         </div>
///         <div id = {id} class = "closable">
///             This content can be hidden.
///         </div>
///     </div>
/// }
/// ```
/// Note that the css classes `closable-container`, `closable-control` and `closable` controls the visibility,
/// but are not used by `toggle_closed`.
pub(crate) fn toggle_closed(element: Option<Element>) {
    if let Err(e) = element
        .expect("Parent element should exist, this should never fail.")
        .class_list()
        .toggle("closed")
    {
        if let Some(js) = e.as_string() {
            logging::warn!("JsValue: {js}");
        } else {
            logging::warn!("Cannot display JsValue error");
        }
    }
}
