use crate::{
    app::{ServerError, SessionError},
    finder::SearchEngine,
    sessions::session::Session,
    structs::{BrokerInfo, SearchTarget, Topics},
};
use std::{collections::HashMap, sync::Arc};
use tokio::{sync::Mutex, time::Duration};
use tracing::{debug, instrument, trace};
use uuid::Uuid;

/// Encapsulates all run-time settings which are needed by the session engine.
#[derive(Default, Clone, Debug)]
pub struct SessionEngineSettings {
    /// Address of the broker.
    pub broker: String,
    /// Topics that are available to the search engine.
    pub topics: Topics,
    pub username: Option<String>,
    pub password: Option<String>,
    /// Kafka consumer group to use.
    pub consumer_group: String,
    /// TTL of any session.
    pub session_ttl_sec: i64,
}

#[derive(Default)]
pub struct SessionEngine {
    /// Settings for this engine.
    settings: SessionEngineSettings,
    /// Currently active search sessions, addressed by their Uuid.
    sessions: HashMap<String, Session>,
}

impl SessionEngine {
    /// Create new session engine, boxed in an `Arc<Mutex<_>>`.
    pub fn with_arc_mutex(settings: SessionEngineSettings) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            settings,
            sessions: Default::default(),
        }))
    }

    /// Generate random Uuid for a session key.
    fn generate_key(&self) -> String {
        let mut key = Uuid::new_v4().to_string();
        while self.sessions.contains_key(&key) {
            key = Uuid::new_v4().to_string();
        }
        key
    }

    /// Create a new search session.
    /// 
    /// # Parameters
    /// - target: search criteria.
    /// - events_topic_indices: list of eventlist topics to search on.
    pub fn create_new_search(
        &mut self,
        target: SearchTarget,
        events_topic_indices: Vec<usize>,
    ) -> Result<String, SessionError> {
        let consumer = digital_muon_common::create_default_consumer(
            &self.settings.broker,
            &self.settings.username,
            &self.settings.password,
            &self.settings.consumer_group,
            None,
        )?;

        let searcher = SearchEngine::new(consumer, &self.settings.topics, events_topic_indices);

        let key = self.generate_key();
        self.sessions.insert(
            key.clone(),
            Session::new_search(searcher, target, self.settings.session_ttl_sec),
        );
        Ok(key)
    }

    /// Get a reference to the session with the corresponding uuid.
    /// 
    /// # Parameters
    /// - uuid: the uuid of the desired session.
    pub fn session(&self, uuid: &str) -> Result<&Session, SessionError> {
        self.sessions.get(uuid).ok_or(SessionError::DoesNotExist)
    }

    /// Get a reference to the settings.
    pub fn settings(&self) -> &SessionEngineSettings {
        &self.settings
    }

    /// Get a mutable reference to the session with the corresponding uuid.
    /// 
    /// # Parameters
    /// - uuid: the uuid of the desired session.
    pub fn session_mut(&mut self, uuid: &str) -> Result<&mut Session, SessionError> {
        self.sessions
            .get_mut(uuid)
            .ok_or(SessionError::DoesNotExist)
    }

    /// Delete any expired sessions from the engine.
    #[instrument(skip_all)]
    pub fn purge_expired(&mut self) {
        let dead_uuids: Vec<String> = self
            .sessions
            .keys()
            .filter(|&uuid| self.sessions.get(uuid).is_some_and(Session::expired))
            .cloned()
            .collect::<Vec<_>>();

        debug!("Purging {} dead session(s)", dead_uuids.len());

        for uuid in dead_uuids {
            self.sessions.remove_entry(&uuid);
        }
    }

    /// Create a thread that owns a copy of an `Arc<Mut<_>>` of the engine, and periodically calls [Self::purge_expired]
    /// to remove old search sessions.
    /// 
    /// # Parameters
    /// - session_engine: `Arc<Mut<_>>` of the engine to purge.
    /// - purge_session_interval_sec: how often to run the purge.
    pub fn spawn_purge_task(
        session_engine: Arc<Mutex<Self>>,
        purge_session_interval_sec: u64,
    ) -> tokio::task::JoinHandle<()> {
        tokio::task::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(purge_session_interval_sec));

            loop {
                interval.tick().await;
                session_engine.lock().await.purge_expired();
            }
        })
    }


    /// Poll the broker for a summary of its contents, on both the trace topic and a specified eventlist topic.
    /// 
    /// # Parameters
    /// - poll_broker_timeout_ms: duration in milliseconds at which the request should timeout.
    /// - events_topic_index: eventlist topic to poll.
    #[instrument(skip_all)]
    pub async fn poll_broker(
        &self,
        poll_broker_timeout_ms: u64,
        events_topic_index: usize,
    ) -> Result<BrokerInfo, ServerError> {
        debug!("Beginning Broker Poll");
        trace!("{:?}", self.settings);

        let consumer = digital_muon_common::create_default_consumer(
            &self.settings.broker,
            &self.settings.username,
            &self.settings.password,
            &self.settings.consumer_group,
            None,
        )?;

        let searcher = SearchEngine::new(consumer, &self.settings.topics, Default::default());

        Ok(searcher
            .poll_broker(poll_broker_timeout_ms, events_topic_index)
            .await?)
    }
}
