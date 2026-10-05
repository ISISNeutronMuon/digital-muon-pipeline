use crate::app::{main_content::MainLevelContext, server_functions::CancelSearch};
use leptos::{IntoView, component, either::Either, prelude::*, view};

/// Panel which allows the user to initiate or cancel a search.
#[component]
pub(crate) fn SearchControl() -> impl IntoView {
    let main_context = use_context::<MainLevelContext>()
        .expect("MainLevelContext should be provided, this should never fail.");
    let await_search = main_context.await_search;
    let uuid = main_context.uuid;

    let cancel_search_server_action = ServerAction::<CancelSearch>::new();

    move || {
        if await_search.pending().get() {
            Either::Left(
                uuid.get().map(move |uuid|view! {
                    <div class = "searching">Searching...</div>
                    <input type = "button" class = "cancel-button" value = "Cancel"
                        on:click = move |_| { cancel_search_server_action.dispatch(CancelSearch { uuid: uuid.clone() }); }
                    />
                })
            )
        } else {
            Either::Right(view! {
                <input type = "submit" class = "search-button" value = "Search" />
            })
        }
    }
}
