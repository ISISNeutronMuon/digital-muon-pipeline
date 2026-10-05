use crate::{
    finder::{
        task::{BinarySearchByTimestamp, Dragnet, SearchTask},
        topic_searcher::{Searcher, SearcherError},
    },
    structs::{
        BrokerInfo, BrokerTopicInfo, EventListMessage, FBMessage, SearchResults, SearchTarget,
        SearchTargetMode, Topics, TraceMessage,
    },
};
use chrono::Utc;
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    error::KafkaError,
    util::Timeout,
};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, instrument};

#[derive(Error, Debug)]
pub(crate) enum SearchEngineError {
    #[error("Searcher Error: {0}")]
    Searcher(#[from] SearcherError),
    #[error("Kafka Error {0}")]
    Kafka(#[from] KafkaError),
}

/// Controls a single search of the broker, either to poll for a summary of contents
/// or to find trace and eventlists.
pub struct SearchEngine {
    /// The Kafka consumer object the engine uses to poll for messages.
    consumer: StreamConsumer,
    /// The known topics.
    topics: Topics,
    /// Indices specifying the subset of eventlist topics the engine should search.
    events_topic_indices: Vec<usize>,
}

impl SearchEngine {
    /// Creates new search engine.
    ///
    /// There should only be one engine per search session or poll broker call.
    ///
    /// # Parameters
    /// - consumer: the Kafka consumer object the engine uses to consume messages.
    /// - topics: Lists of all known topics.
    /// - events_topic_indices: list of indices specifying the subset of eventlist topics to search on.
    pub fn new(
        consumer: StreamConsumer,
        topics: &Topics,
        events_topic_indices: Vec<usize>,
    ) -> Self {
        Self {
            consumer,
            topics: topics.clone(),
            events_topic_indices,
        }
    }

    /// Gets the summary for the given topic.
    ///
    /// # Parameters
    /// - consumer: the Kafka consumer object the engine uses to consume messages.
    /// - topic: topic to summarise.
    /// - poll_broker_timeout_ms: duration in milliseconds, in which the request should be timed out.
    async fn poll_broker_topic_info<'a, M: FBMessage<'a>>(
        consumer: &'a StreamConsumer,
        topic: &str,
        poll_broker_timeout_ms: u64,
    ) -> Result<BrokerTopicInfo, SearchEngineError> {
        let offsets = consumer.fetch_watermarks(
            topic,
            0,
            Timeout::After(Duration::from_millis(poll_broker_timeout_ms)),
        )?;
        debug!("Topic {topic}: (High, Low) offsets: {offsets:?}");

        if offsets.0 == offsets.1 {
            Ok(BrokerTopicInfo {
                offsets,
                timestamps: None,
            })
        } else {
            let mut searcher = Searcher::<M, StreamConsumer>::new(consumer, topic, offsets.0)?;
            let begin = searcher.message(offsets.0).await?;
            let end = searcher.message(offsets.1 - 1).await?;

            Ok(BrokerTopicInfo {
                offsets,
                timestamps: Some((begin.timestamp(), end.timestamp())),
            })
        }
    }

    /// Execute a poll broker request to obtain a summary of the trace topic and a single eventlist topic.
    ///
    /// # Parameters
    /// - poll_broker_timeout_ms: duration in milliseconds, in which the request should be timed out.
    /// - events_topic_index: index of the eventlist topics to summarise.
    #[instrument(skip_all)]
    pub(crate) async fn poll_broker(
        &self,
        poll_broker_timeout_ms: u64,
        events_topic_index: usize,
    ) -> Result<BrokerInfo, SearchEngineError> {
        let trace = Self::poll_broker_topic_info::<TraceMessage>(
            &self.consumer,
            &self.topics.trace_topic,
            poll_broker_timeout_ms,
        )
        .await?;

        let events_topic = self
            .topics
            .digitiser_event_topic
            .get(events_topic_index)
            .expect("event topic index should be in range, this should never fail.");
        let events = Self::poll_broker_topic_info::<EventListMessage>(
            &self.consumer,
            events_topic,
            poll_broker_timeout_ms,
        )
        .await?;

        Ok(BrokerInfo {
            timestamp: Utc::now(),
            trace,
            events,
        })
    }

    /// Execute a search of the broker using the given target.
    ///
    /// # Parameters
    /// - target: the search criteria and settings.
    #[instrument(skip_all)]
    pub(crate) async fn search(
        &mut self,
        target: SearchTarget,
    ) -> Result<SearchResults, SearchEngineError> {
        Ok(match target.mode {
            SearchTargetMode::Timestamp { timestamp } => {
                SearchTask::<BinarySearchByTimestamp>::new(
                    &self.consumer,
                    &self.topics,
                    self.events_topic_indices.clone(),
                )
                .search(timestamp, target.by, target.number)
                .await?
            }
            SearchTargetMode::Dragnet {
                timestamp,
                backstep,
                forward_distance,
            } => {
                SearchTask::<Dragnet>::new(
                    &self.consumer,
                    &self.topics,
                    self.events_topic_indices.clone(),
                )
                .search(
                    timestamp,
                    backstep,
                    forward_distance,
                    target.by,
                    target.number,
                )
                .await?
            }
        })
    }
}
