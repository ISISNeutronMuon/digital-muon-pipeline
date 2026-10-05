use crate::{
    Timestamp,
    app::SessionError,
    finder::SearchEngine,
    structs::{
        DigitiserMetadata, DigitiserTrace, SearchResults, SearchSummary, SearchTarget, TraceSummary,
    },
};
use chrono::{TimeDelta, Utc};
use tokio::{sync::oneshot, task::JoinHandle};
use tracing::instrument;

/// Contains a handle to the thread conducting the search, as well as the
pub struct SessionSearchBody {
    pub(crate) handle: JoinHandle<Result<SearchResults, SessionError>>,
    pub(crate) cancel_recv: oneshot::Receiver<()>,
}

/// Encapsulates a session which exists whilst a search is in progress.
pub struct Session {
    /// Search criteria.
    target: SearchTarget,
    /// Search Results.
    results: Option<SearchResults>,
    /// Component of a session that can be transferred between threads.
    search_body: Option<SessionSearchBody>,
    /// One-shot channel whose use cancels the search currently in progress.
    cancel_send: Option<oneshot::Sender<()>>,
    /// Time at which the session expires.
    ///
    /// This is set at creation, but can be refreshed by calling [Self::refresh].
    expiration: Timestamp,
    /// TTL for the session.
    session_ttl: TimeDelta,
}

impl Session {
    /// Default time, in minutes, afterwhich the session should expire.
    const EXPIRE_TIME_MIN: i64 = 10;

    /// Create a new search session.
    ///
    /// # Parameters
    /// - searcher: engine to use for the search.
    /// - target: search criteria.
    /// - session_ttl_sec: ttl for this new session.
    pub(crate) fn new_search(
        mut searcher: SearchEngine,
        target: SearchTarget,
        session_ttl_sec: i64,
    ) -> Self {
        let (cancel_send, cancel_recv) = oneshot::channel();
        Session {
            target: target.clone(),
            results: None,
            search_body: Some(SessionSearchBody {
                handle: tokio::task::spawn(async move { Ok(searcher.search(target).await?) }),
                cancel_recv,
            }),
            cancel_send: Some(cancel_send),
            expiration: Utc::now() + TimeDelta::minutes(Self::EXPIRE_TIME_MIN),
            session_ttl: TimeDelta::seconds(session_ttl_sec),
        }
    }

    #[instrument(skip_all)]
    pub fn take_search_body(&mut self) -> Result<SessionSearchBody, SessionError> {
        self.search_body
            .take()
            .ok_or(SessionError::BodyAlreadyTaken)
    }

    /// Cancel search in progress.
    ///
    /// This should only be called once per session, calling this on an already cancelled session
    /// results in `SessionError::AttemptedToCancelTwice`.
    #[instrument(skip_all)]
    pub fn cancel(&mut self) -> Result<(), SessionError> {
        self.cancel_send
            .take()
            .ok_or(SessionError::AttemptedToCancelTwice)?
            .send(())
            .map_err(|_| SessionError::CouldNotSendCancelSignal)
    }

    /// Takes ownership of the given results.
    #[instrument(skip_all)]
    pub(crate) fn register_results(&mut self, result: SearchResults) {
        self.results = Some(result);
    }

    /// Creates a search summary for each digitiser message in the search results.
    #[instrument(skip_all)]
    pub fn get_search_summaries(&self) -> Result<SearchSummary, SessionError> {
        let cache = self
            .results
            .as_ref()
            .ok_or(SessionError::ResultsMissing)?
            .cache()?;
        let traces = cache
            .iter()
            .enumerate()
            .map(|(index, (metadata, trace))| {
                let date = metadata
                    .timestamp
                    .date_naive()
                    .format("%y-%m-%d")
                    .to_string();
                let time = metadata.timestamp.time().format("%H:%M:%S.%f").to_string();
                let frame_number = metadata.frame_number;
                let period_number = metadata.period_number;
                let protons_per_pulse = metadata.protons_per_pulse;
                let running = metadata.running;
                let veto_flags = metadata.veto_flags;
                let id = metadata.id;
                let channels = trace.traces.keys().copied().collect::<Vec<_>>();
                TraceSummary {
                    date,
                    time,
                    frame_number,
                    period_number,
                    protons_per_pulse,
                    running,
                    veto_flags,
                    index,
                    id,
                    channels,
                }
            })
            .collect::<Vec<_>>();
        Ok(SearchSummary {
            eventlist_topic_indices: cache.get_eventlist_topic_indices().copied().collect(),
            target: self.target.clone(),
            traces,
        })
    }

    /// Gets a reference to the digitiser message stored at the given index.
    ///
    /// # Parameters
    /// - index: index of the stored digitiser message to obtain.
    pub(crate) fn get_selected_trace(
        &self,
        index: usize,
    ) -> Result<(&DigitiserMetadata, &DigitiserTrace), SessionError> {
        self.results
            .as_ref()
            .ok_or(SessionError::ResultsMissing)?
            .cache()?
            .iter()
            .nth(index)
            .ok_or(SessionError::TraceNotFound)
    }

    /// Checks whether the session is expired.
    pub(crate) fn expired(&self) -> bool {
        self.expiration < Utc::now()
    }

    /// Refreshes the session's expiration date to [Self::session_ttl] after [Utc::now()].
    pub(crate) fn refresh(&mut self) {
        self.expiration = Utc::now() + self.session_ttl
    }
}
