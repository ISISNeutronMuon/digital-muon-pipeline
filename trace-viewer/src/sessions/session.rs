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

pub struct SessionSearchBody {
    pub(crate) handle: JoinHandle<Result<SearchResults, SessionError>>,
    pub(crate) cancel_recv: oneshot::Receiver<()>,
}

pub struct Session {
    target: SearchTarget,
    results: Option<SearchResults>,
    search_body: Option<SessionSearchBody>,
    cancel_send: Option<oneshot::Sender<()>>,
    expiration: Timestamp,
    session_ttl: TimeDelta,
}

impl Session {
    const EXPIRE_TIME_MIN: i64 = 10;

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

    #[instrument(skip_all)]
    pub fn cancel(&mut self) -> Result<(), SessionError> {
        self.cancel_send
            .take()
            .ok_or(SessionError::AttemptedToCancelTwice)?
            .send(())
            .map_err(|_| SessionError::CouldNotSendCancelSignal)
    }

    #[instrument(skip_all)]
    pub fn register_results(&mut self, result: SearchResults) {
        self.results = Some(result);
    }

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

    pub(crate) fn expired(&self) -> bool {
        self.expiration < Utc::now()
    }

    pub(crate) fn refresh(&mut self) {
        self.expiration = Utc::now() + self.session_ttl
    }
}
