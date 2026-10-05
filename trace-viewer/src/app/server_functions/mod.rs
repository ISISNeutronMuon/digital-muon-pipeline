//! All server functions appear here.
mod errors;
mod plotly;
mod search;

use crate::structs::{BrokerInfo, ClientSideData};
use cfg_if::cfg_if;
use leptos::prelude::*;
use tracing::instrument;

pub use plotly::CreateAndFetchPlotly;
pub use search::{AwaitSearch, CancelSearch, CreateNewSearch, FetchSearchSummaries};

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use crate::structs::ServerSideData;
        use tracing::debug;

        pub(crate) use errors::{SessionError, ServerError};
    }
}

#[server]
#[instrument(skip_all)]
pub async fn get_client_side_data() -> Result<ClientSideData, ServerFnError> {
    // The mutex should be in scope to apply a lock.
    Ok(use_context::<ClientSideData>()
        .expect("Client-side data should be provided, this should never fail."))
}

/// Server function which runs an ActionForm with inputs: integer-valued input with name `events_topic_index`,
/// integer-valued input with name `poll_broker_timeout_ms`.
/// 
/// # Parameters
/// - poll_broker_timeout_ms: duration in milliseconds, in which the request should be timed out.
/// - events_topic_index: index of the eventlist topics to summarise.
#[server]
#[instrument(skip_all)]
pub async fn poll_broker(
    poll_broker_timeout_ms: u64,
    events_topic_index: usize,
) -> Result<BrokerInfo, ServerFnError> {
    // The mutex should be in scope to apply a lock.
    let session_engine_arc_mutex = use_context::<ServerSideData>()
        .expect("ServerSideData should be provided, this should never fail.")
        .session_engine;

    let session_engine = session_engine_arc_mutex.lock().await;

    let broker_info = session_engine
        .poll_broker(poll_broker_timeout_ms, events_topic_index)
        .await?;

    Ok(broker_info)
}

/// Refreshes the expiration of the session with the given uuid.
/// 
/// # Parameters
/// - uuid: uuid of the search session to fetch from.
#[server]
#[instrument(skip_all, err(level = "warn"))]
pub async fn refresh_session(uuid: String) -> Result<(), ServerFnError> {
    let session_engine_arc_mutex = use_context::<ServerSideData>()
        .expect("ServerSideData should be provided, this should never fail.")
        .session_engine;

    let mut session_engine = session_engine_arc_mutex.lock().await;

    let session = session_engine.session_mut(&uuid)?;
    session.refresh();
    debug!("Session {uuid} refreshed.");
    Ok(())
}
