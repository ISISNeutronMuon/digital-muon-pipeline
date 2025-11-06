use crate::structs::{SearchSummary, SearchTarget};
use cfg_if::cfg_if;
use leptos::prelude::*;
use tracing::instrument;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use crate::structs::{SearchResults, ServerSideData};
        use tracing::{debug, error};
    }
}

/// Creates a new search session and returns the [Uuid].
#[server]
#[instrument(skip_all, err(level = "warn"))]
pub async fn create_new_search(target: SearchTarget, events_topic_index: usize) -> Result<String, ServerFnError> {
    debug!("Creating new search task for target: {:?}", target);

    // The mutex should be in scope to apply a lock.
    let session_engine_arc_mutex = use_context::<ServerSideData>()
        .expect("ServerSideData should be provided, this should never fail.")
        .session_engine;

    let mut session_engine = session_engine_arc_mutex.lock().await;

    let uuid = session_engine.create_new_search(target, events_topic_index)?;

    debug!("New search task has uuid: {}", uuid);

    Ok(uuid)
}

/// Sends the one-shop cancel message to the [Session] with the given [Uuid].
/// Returns an error if no such session exists.
#[server]
pub async fn cancel_search(uuid: String) -> Result<(), ServerFnError> {
    // The mutex should be in scope to apply a lock.
    let session_engine_arc_mutex = use_context::<ServerSideData>()
        .expect("ServerSideData should be provided, this should never fail.")
        .session_engine;
    let mut session_engine = session_engine_arc_mutex.lock().await;

    let session = session_engine.session_mut(&uuid)?;
    session.cancel()?;
    Ok(())
}

/// Takes ownership of the search body of the [Session] with the given [Uuid],
/// and waits for it's [JoinHandle] field to complete, or is cancelled.
/// If it completes then it registers the results with the original [Session].
/// Returns an error if no such session exists.
#[server]
#[instrument(skip_all, err(level = "warn"))]
pub async fn await_search(uuid: String) -> Result<String, ServerFnError> {
    use crate::sessions::SessionSearchBody;

    // Obtain SessionSearchBody without locking SessionEngine for too long.
    let SessionSearchBody {
        handle,
        cancel_recv,
    } = {
        let session_engine_arc_mutex = use_context::<ServerSideData>()
            .expect("ServerSideData should be provided, this should never fail.")
            .session_engine;

        let mut session_engine = session_engine_arc_mutex.lock().await;

        let session = session_engine.session_mut(&uuid)?;
        session.take_search_body()?
    };

    // Run Future
    tokio::select! {
        results = handle => {
            let results = results
                .inspect(|_| debug!("Successfully found results."))
                .or_else(|e| if e.is_cancelled() { Ok(Ok(SearchResults::Cancelled)) } else { Err(e) })??;

            // Register results with SessionEngine and return results.
            let session_engine_arc_mutex = use_context::<ServerSideData>()
                .expect("ServerSideData should be provided, this should never fail.")
                .session_engine;

            let mut session_engine = session_engine_arc_mutex
                .lock()
                .await;

            session_engine.session_mut(&uuid)?
                .register_results(results);
        }
        result = cancel_recv => {
            if let Err(e) = result {
                error!("{}",e);
            }
        }
    }

    Ok(uuid)
}

/// Fetches the list of summaries of messages in the cache of the session with the given [Uuid].
/// Returns an error if no such session exists.
#[server]
#[instrument(skip_all, err(level = "warn"))]
pub async fn fetch_search_summaries(uuid: String) -> Result<SearchSummary, ServerFnError> {
    let session_engine_arc_mutex = use_context::<ServerSideData>()
        .expect("ServerSideData should be provided, this should never fail.")
        .session_engine;

    let session_engine = session_engine_arc_mutex.lock().await;

    let session = session_engine.session(&uuid)?;

    Ok(session.get_search_summaries()?)
}
