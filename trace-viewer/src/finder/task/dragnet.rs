use crate::{
    Timestamp,
    finder::{
        task::{SearchTask, TaskClass},
        topic_searcher::{Searcher, SearcherError},
    },
    structs::{Cache, EventListMessage, FBMessage, SearchResults, SearchTargetBy, TraceMessage},
};
use rdkafka::consumer::StreamConsumer;
use tracing::{info, instrument};

/// Allows for the subtype `SearchTask<'a, Dragnet>`.
pub(crate) struct Dragnet;
impl TaskClass for Dragnet {}

impl<'a> SearchTask<'a, Dragnet> {
    /// Performs a dragnet search, with generic filtering functions.
    /// 
    /// This begins with a binary tree search followed by a search window focussed about the binary tree search result.
    /// 
    /// # Parameters
    /// - searcher: the topic-specific [Searcher].
    /// - target: timestamp to search for initially.
    /// - backstep: number of messages to jump back.
    /// - forward_distance: how far forwards to search for matching messages.
    /// - number: the maximum number of results to match.
    /// - aquire_while: a generic filtering function.
    #[instrument(skip_all)]
    async fn search_topic<M, A>(
        &self,
        searcher: Searcher<'a, M, StreamConsumer>,
        target: Timestamp,
        backstep: i64,
        forward_distance: usize,
        number: usize,
        acquire_matches: A,
    ) -> Option<(Vec<M>, Vec<Timestamp>, i64)>
    where
        M: FBMessage<'a>,
        A: Fn(&M) -> bool,
    {
        let mut iter = searcher.iter_binary(target);
        iter.init().await;

        if iter.empty() {
            return None;
        }
        
        info!("Beginning Binary Search.");
        loop {
            if iter
                .bisect()
                .await
                .expect("bisect works, this should never fail.")
            {
                break;
            }
        }

        let searcher = iter.collect();
        let offset = searcher.get_offset();

        info!("Beginning Dragnet Search.");
        let mut iter = searcher.iter_dragnet(number);
        iter.backstep_by(backstep)
            .acquire_matches(forward_distance, acquire_matches)
            .await;
        let (searcher, timestamps) = iter.collect();
        let results: Vec<M> = searcher.into();

        Some((results, timestamps, offset))
    }

    /// Performs a dragnet search.
    /// 
    /// This begins with a binary tree search followed by a search window focussed about the binary tree search result.
    /// 
    /// # Parameters
    /// - target: timestamp to search for initially.
    /// - backstep: number of messages to jump back.
    /// - forward_distance: how far forwards to search for matching messages.
    /// - search_by: what criteria to match on.
    /// - number: the maximum number of results to match.
    #[instrument(skip_all)]
    pub(crate) async fn search(
        self,
        target_timestamp: Timestamp,
        backstep: i64,
        forward_distance: usize,
        search_by: SearchTargetBy,
        number: usize,
    ) -> Result<SearchResults, SearcherError> {
        // Find Digitiser Traces
        let searcher = Searcher::new(self.consumer, &self.topics.trace_topic, 1)?;

        let trace_results = self
            .search_topic(
                searcher,
                target_timestamp,
                backstep,
                forward_distance,
                number,
                |msg: &TraceMessage| msg.filter_by(&search_by),
            )
            .await;

        let mut cache = Cache::new();

        if let Some((trace_results, timestamps, offset)) = trace_results {
            info!("Found {} trace(s).", trace_results.len());
            for trace in trace_results.iter() {
                cache.push_trace(
                    &trace
                        .try_unpacked_message()
                        .expect("Cannot Unpack Trace. TODO should be handled"),
                )?;
            }

            for &index in self.events_topic_indices.iter() {
                let event_topic = self
                    .topics
                    .digitiser_event_topic
                    .get(index)
                    .expect("event topic index should be in range, this should never fail.");

                // Find Digitiser Event Lists
                let searcher = Searcher::new(self.consumer, event_topic, offset)?;
                let digitiser_ids = Self::get_digitiser_ids_from_traces(trace_results.as_slice());
                let eventlist_results = self
                    .search_topic(
                        searcher,
                        target_timestamp,
                        backstep,
                        forward_distance,
                        timestamps.len(),
                        |msg: &EventListMessage| {
                            msg.filter_by_digitiser_id(&digitiser_ids)
                                && timestamps.contains(&msg.timestamp())
                        },
                    )
                    .await;

                if let Some((eventlist_results, _, _)) = eventlist_results {
                    for eventlist in eventlist_results.iter() {
                        cache.push_events(
                            index,
                            &eventlist
                                .try_unpacked_message()
                                .expect("Cannot Unpack Eventlist. TODO should be handled"),
                        )?;
                    }
                }
            }
        }
        cache.attach_event_lists_to_trace();

        Ok(SearchResults::Successful { cache })
    }
}
