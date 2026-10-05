mod borrowed_messages;
mod search_results;

use crate::sessions::SessionEngine;
use std::sync::Arc;
use tokio::sync::Mutex;

pub(crate) use borrowed_messages::{
    BorrowedMessageError, EventListMessage, FBMessage, TraceMessage,
};
pub(crate) use search_results::{Cache, SearchResults};

/// Encapsulates all run-time settings which are only available to the server.
#[derive(Default, Clone)]
pub struct ServerSideData {
    /// Controls the search sessions.
    pub session_engine: Arc<Mutex<SessionEngine>>,
}
