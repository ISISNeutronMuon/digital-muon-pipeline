use leptos::{logging, prelude::*};
use leptos_use::use_interval;

use crate::{
    Uuid,
    app::{
        sections::{BrokerSection, ResultsSection, SearchSection},
        server_functions::{AwaitSearch, CreateNewSearch, FetchSearchSummaries, RefreshSession},
    },
};

/// This struct enable a degree of type-checking for the [use_context]/[use_context] functions.
/// Any component making use of the following fields should call `use_context::<MainLevelContext>()`
/// and select the desired field.
#[derive(Clone)]
pub(crate) struct MainLevelContext {
    pub(crate) create_new_search: ServerAction<CreateNewSearch>,
    pub(crate) await_search: ServerAction<AwaitSearch>,
    pub(crate) fetch_search_search: ServerAction<FetchSearchSummaries>,
    pub(crate) uuid: Signal<Uuid>,
}

/// Creates the body of the page below the [TopBar].
///
/// Creates and provides the top-level [ServerActions] and signals.
#[component]
pub(crate) fn Main() -> impl IntoView {
    let create_new_search = ServerAction::<CreateNewSearch>::new();
    // Derived signal which collects the `Uuid` when `create_new_search` finishes, and
    // emits a warning if the result is an `Err`.
    let uuid = Signal::derive(move || {
        create_new_search
            .value()
            .get()
            .and_then(|uuid| uuid.inspect_err(|e| logging::warn!("{e}")).ok())
    });
    provide_context(MainLevelContext {
        create_new_search,
        uuid,
        await_search: ServerAction::new(),
        fetch_search_search: ServerAction::new(),
    });

    init_search_control_effects();
    init_refresh_session_effect();

    view! {
        <div class = "main">
            <SearchSection />
            <ResultsSection />
            <BrokerSection />
        </div>
    }
}

/// Creates the [ServerAction]s which create, run, and collect results from, a search job,
/// and the [Effect]s through which they interact.
/// - When `create_new_search` is pending, then `await_search` and `fetch_search_summaries` are cleared.
/// - When `uuid` updates, then `await_search` is dispatched. Note that `uuid` updates whenever `create_new_search` completes.
/// - When `await_search` finishes, then (after error handling), `fetch_search_summaries` is dispatched.
fn init_search_control_effects() {
    let main_context = use_context::<MainLevelContext>()
        .expect("MainLevelContext should be provided, this should never fail.");
    let create_new_search = main_context.create_new_search;
    let await_search = main_context.await_search;
    let fetch_search_summaries = main_context.fetch_search_search;
    let uuid = main_context.uuid;

    // Clear await_search and fetch_search_summaries when a new search is created.
    Effect::new(move || {
        if create_new_search.pending().get() {
            await_search.clear();
            fetch_search_summaries.clear();
        }
    });

    // Call await search when a new uuid is created.
    Effect::new(move || {
        if let Some(uuid) = uuid.get() {
            await_search.dispatch(AwaitSearch { uuid });
        }
    });

    // Fetch summaries when await_search is finished.
    Effect::new(move || match await_search.value().get() {
        Some(Ok(uuid)) => {
            fetch_search_summaries.dispatch(FetchSearchSummaries { uuid });
        }
        Some(Err(e)) => logging::warn!("{e}"),
        _ => {}
    });
}

/// Creates: the [ServerAction] to refresh a session with a given `uuid`,
/// an interval timer which triggers every 30,000 ms, and
/// an effect which dispatches the action when the timer triggers.
fn init_refresh_session_effect() {
    let main_context = use_context::<MainLevelContext>()
        .expect("MainLevelContext should be provided, this should never fail.");
    let uuid = main_context.uuid;

    let refresh_session = ServerAction::<RefreshSession>::new();

    let refresh_interval = use_interval(30_000);
    Effect::new(move || {
        if let Some(uuid) = uuid.get() {
            refresh_interval.counter.track();
            refresh_session.dispatch(RefreshSession { uuid });
        }
    });
}
