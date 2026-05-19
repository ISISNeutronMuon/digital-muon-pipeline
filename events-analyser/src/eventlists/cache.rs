//! Defines the cache stores frames as they are assembled from digitiser messages.
use crate::{event::EventData, eventlists::partial::EventlistsCollection};

use super::{RejectMessageError, partial::PartialEventslistsCollection};
use digital_muon_common::{
    DigitizerId, spanned::SpannedAggregator
};
use digital_muon_streaming_types::FrameMetadata;
use std::{collections::VecDeque, time::Duration};
use tracing::warn;

/// Contains all the partial frames as well as handling the frame lifetime and completeness.
pub(crate) struct MessageCache {
    /// Specifies the maximum time that a partial frame should live
    /// in the cache before being dispatched event if it is missing some digitisers.
    ttl: Duration,
    num_topics: usize,
    eventlists: VecDeque<PartialEventslistsCollection>,
}

impl MessageCache {
    /// Creates and returns a new [FrameCache] instance.
    /// # Parameters
    /// - ttl: time-to-live duration
    /// - expected_digitisers: list of digitisers that form a complete frame.
    ///
    /// Note that `expected_digitisers` should be increasing and without duplicates, this is not checked.
    pub(crate) fn new(ttl: Duration, num_topics: usize) -> Self {
        Self {
            ttl,
            num_topics,
            eventlists: Default::default()
        }
    }

    /// Pushes the contents of a new digitiser message into the cache.
    /// If a partial frame with the same `metadata` already exists, and is yet
    /// to receive a message with the same `digitiser_id`, then `data` is added
    /// to the partial frame, otherwise a new [PartialFrame] is created.
    #[tracing::instrument(skip_all, level = "trace")]
    pub(crate) fn push(
        &mut self,
        digitiser_id: DigitizerId,
        metadata: &FrameMetadata,
        topic_index: usize,
        data: EventData,
    ) -> Result<(), RejectMessageError> {
        let frame_dig = {
            match self
                .eventlists
                .iter_mut()
                .find(|frame_dig: &&mut PartialEventslistsCollection| frame_dig.metadata.equals_ignoring_veto_flags(metadata) && frame_dig.digitiser_id == digitiser_id)
            {
                Some(frame_dig) => {
                    frame_dig.push(topic_index, data);
                    frame_dig
                }
                None => {
                    let mut frame_dig = PartialEventslistsCollection::new(self.num_topics, self.ttl, metadata, digitiser_id);

                    // Initialise the span field
                    /*if let Err(e) = frame_dig.span_init() {
                        warn!("Frame span initiation failed {e}")
                    }*/

                    frame_dig.push(topic_index, data);
                    self.eventlists.push_back(frame_dig);
                    self.eventlists
                        .back()
                        .expect("self.frames should be non-empty, this should never fails")
                }
            }
        };

        Ok(())
    }

    /// Checks whether any partial frame is ready to be dispatched, that is either
    /// has a complete complement of digitisers, or has been in the cache past its expiry time.
    /// If one is found it is removed from the cache and returned as an [AggregatedFrame].
    pub(crate) fn poll(&mut self) -> Option<EventlistsCollection> {
        // Find a frame which is completed
        if self
            .eventlists
            .front()
            .is_some_and(|frame_dig: &PartialEventslistsCollection| frame_dig.is_complete() || frame_dig.is_expired())
        {
            let frame_dig = self
                .eventlists
                .pop_front()
                .expect("self.frames should be non-empty, this should never fail");
            if let Err(e) = frame_dig.end_span() {
                warn!("Frame span drop failed {e}")
            }

            frame_dig.try_complete()
        } else {
            None
        }
    }

    /// Returns the number of partial frames currently in the cache.
    pub(crate) fn get_num_partial_frames(&self) -> usize {
        self.eventlists.len()
    }
}
/*
#[cfg(test)]
mod test {
    use super::*;
    use crate::data::EventData;
    use chrono::Utc;

    #[test]
    fn one_frame_in_one_frame_out() {
        let mut cache = MessageCache::<EventData>::new(Duration::from_millis(100), 1);

        let frame_1 = FrameMetadata {
            timestamp: Utc::now(),
            period_number: 1,
            protons_per_pulse: 8,
            running: true,
            frame_number: 1728,
            veto_flags: 4,
        };

        assert!(cache.poll().is_none());

        assert_eq!(cache.get_num_partial_frames(), 0);
        assert!(
            cache
                .push(0, frame_1.clone(), 0, EventData::dummy_data(0, 5, &[0, 1, 2]))
                .is_ok()
        );
        assert_eq!(cache.get_num_partial_frames(), 1);

        assert!(cache.poll().is_none());

        assert!(
            cache
                .push(1, frame_1, 0, EventData::dummy_data(0, 5, &[3, 4, 5]))
                .is_ok()
        );

        assert!(cache.poll().is_none());

        assert!(
            cache
                .push(4, &frame_1, 0, EventData::dummy_data(0, 5, &[6, 7, 8]))
                .is_ok()
        );

        assert!(cache.poll().is_none());

        assert!(
            cache
                .push(8, &frame_1, 0, EventData::dummy_data(0, 5, &[9, 10, 11]))
                .is_ok()
        );

        {
            let frame = cache.poll().unwrap();
            assert_eq!(cache.get_num_partial_frames(), 0);

            assert_eq!(frame.metadata, frame_1);

            let mut dids = frame.digitiser_ids;
            dids.sort();
            assert_eq!(dids, &[0, 1, 4, 8]);

            assert_eq!(
                frame.digitiser_data,
                EventData::new(
                    vec![
                        0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4,
                        0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4,
                        0, 1, 2, 3, 4, 0, 1, 2, 3, 4
                    ],
                    vec![0; 60],
                    vec![
                        0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4,
                        5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9,
                        10, 10, 10, 10, 10, 11, 11, 11, 11, 11
                    ],
                )
            );
        }

        assert!(cache.poll().is_none());
    }

    #[tokio::test]
    async fn one_frame_in_one_frame_out_missing_digitiser_timeout() {
        let mut cache = MessageCache::<EventData>::new(Duration::from_millis(100), vec![0, 1, 4, 8]);

        let frame_1 = FrameMetadata {
            timestamp: Utc::now(),
            period_number: 1,
            protons_per_pulse: 8,
            running: true,
            frame_number: 1728,
            veto_flags: 4,
        };

        assert!(cache.poll().is_none());

        assert!(
            cache
                .push(0, &frame_1, EventData::dummy_data(0, 5, &[0, 1, 2]))
                .is_ok()
        );

        assert!(cache.poll().is_none());

        assert!(
            cache
                .push(1, &frame_1, EventData::dummy_data(0, 5, &[3, 4, 5]))
                .is_ok()
        );

        assert!(cache.poll().is_none());

        assert!(
            cache
                .push(8, &frame_1, EventData::dummy_data(0, 5, &[9, 10, 11]))
                .is_ok()
        );

        assert!(cache.poll().is_none());

        tokio::time::sleep(Duration::from_millis(105)).await;

        {
            let frame = cache.poll().unwrap();

            assert_eq!(frame.metadata, frame_1);

            let mut dids = frame.digitiser_ids;
            dids.sort();
            assert_eq!(dids, &[0, 1, 8]);

            assert_eq!(
                frame.digitiser_data,
                EventData::new(
                    vec![
                        0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4,
                        0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4, 0, 1, 2, 3, 4,
                    ],
                    vec![0; 45],
                    vec![
                        0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4,
                        5, 5, 5, 5, 5, 9, 9, 9, 9, 9, 10, 10, 10, 10, 10, 11, 11, 11, 11, 11
                    ],
                )
            );
        }

        assert!(cache.poll().is_none());
    }

    #[tokio::test]
    async fn one_frame_in_one_frame_out_missing_digitiser_and_late_message_timeout() {
        let mut cache = FrameCache::<EventData>::new(Duration::from_millis(100), vec![0, 1, 4, 8]);

        let frame_1 = FrameMetadata {
            timestamp: Utc::now(),
            period_number: 1,
            protons_per_pulse: 8,
            running: true,
            frame_number: 1728,
            veto_flags: 4,
        };
        assert!(
            cache
                .push(0, &frame_1, EventData::dummy_data(0, 5, &[0, 1, 2]))
                .is_ok()
        );
        assert!(
            cache
                .push(1, &frame_1, EventData::dummy_data(0, 5, &[3, 4, 5]))
                .is_ok()
        );
        assert!(
            cache
                .push(8, &frame_1, EventData::dummy_data(0, 5, &[9, 10, 11]))
                .is_ok()
        );

        tokio::time::sleep(Duration::from_millis(105)).await;

        let _ = cache.poll().unwrap();

        //  This call to push should return an error
        assert!(
            cache
                .push(4, &frame_1, EventData::dummy_data(0, 5, &[6, 7, 8]))
                .is_err()
        );
    }

    #[test]
    fn test_metadata_equality() {
        let mut cache = FrameCache::<EventData>::new(Duration::from_millis(100), vec![1, 2]);

        let timestamp = Utc::now();
        let frame_1 = FrameMetadata {
            timestamp,
            period_number: 1,
            protons_per_pulse: 8,
            running: true,
            frame_number: 1728,
            veto_flags: 4,
        };

        let frame_2 = FrameMetadata {
            timestamp,
            period_number: 1,
            protons_per_pulse: 8,
            running: true,
            frame_number: 1728,
            veto_flags: 5,
        };

        assert_eq!(frame_1, frame_2);

        assert_eq!(cache.frames.len(), 0);
        assert!(cache.poll().is_none());

        assert!(
            cache
                .push(1, &frame_1, EventData::dummy_data(0, 5, &[0, 1, 2]))
                .is_ok()
        );
        assert_eq!(cache.frames.len(), 1);
        assert!(cache.poll().is_none());

        assert!(
            cache
                .push(2, &frame_2, EventData::dummy_data(0, 5, &[0, 1, 2]))
                .is_ok()
        );
        assert_eq!(cache.frames.len(), 1);
        assert!(cache.poll().is_some());
    }
}
 */