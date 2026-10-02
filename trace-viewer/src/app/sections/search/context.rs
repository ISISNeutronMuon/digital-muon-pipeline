use crate::{
    Channel, DigitizerId, Timestamp,
    app::sections::search::search_settings::{SearchBy, SearchMode},
    structs::DefaultData,
};
use chrono::{NaiveDate, NaiveTime, Utc};
use leptos::prelude::*;

/// This struct enable a degree of type-checking for the [use_context]/[use_context] functions.
/// Any component making use of the following fields should call `use_context::<SearchLevelContext>()`
/// and select the desired field.
#[derive(Clone)]
pub(crate) struct SearchLevelContext {
    /// Each flag determines whether the corresponding eventlist topic should be searched.
    pub(crate) eventlist_sources: Vec<RwSignal<bool>>,
    /// The selected `SearchMode`.
    pub(crate) search_mode: RwSignal<SearchMode>,
    /// The selected `SearchBy` mode.
    pub(crate) search_by: RwSignal<SearchBy>,
    /// The date criteria search field.
    pub(crate) date: RwSignal<NaiveDate>,
    /// The time criteria search field.
    pub(crate) time: RwSignal<NaiveTime>,
    /// The selected channels to search for.
    pub(crate) channels: RwSignal<Vec<Channel>>,
    /// The selected digitiser ids to search for.
    pub(crate) digitiser_ids: RwSignal<Vec<DigitizerId>>,
    /// The maximum number of messages to return.
    pub(crate) number: RwSignal<usize>,
    /// The backstep field (used in `SearchMode::Dragnet`).
    pub(crate) backstep: RwSignal<i64>,
    /// The forward distance field (used in `SearchMode::Dragnet`).
    pub(crate) forward_distance: RwSignal<usize>,
}

impl SearchLevelContext {
    /// Creates new context from the given `DefaultData` and the number of eventlist topics.
    /// 
    /// # Parameters
    /// - default_data:
    /// - num_eventlist_topics
    pub(crate) fn new(default_data: &DefaultData, num_eventlist_topics: usize) -> Self {
        let default_timestamp = default_data.timestamp.unwrap_or_else(Utc::now);
        let default_date = default_timestamp.date_naive();
        let default_time = default_timestamp.time();
        let search_by = if default_data.channels.is_some() {
            SearchBy::ByChannels
        } else if default_data.digitiser_ids.is_some() {
            SearchBy::ByDigitiserIds
        } else {
            SearchBy::All
        };

        Self {
            eventlist_sources: (0..1)
                .map(|_| RwSignal::new(true))
                .chain((1..num_eventlist_topics).map(|_| RwSignal::new(false)))
                .collect(),
            search_mode: RwSignal::new(SearchMode::default()),
            search_by: RwSignal::new(search_by),
            channels: RwSignal::new(default_data.channels.clone().unwrap_or_default()),
            digitiser_ids: RwSignal::new(default_data.digitiser_ids.clone().unwrap_or_default()),
            date: RwSignal::new(default_date),
            time: RwSignal::new(default_time),
            number: RwSignal::new(default_data.number.unwrap_or(1)),
            backstep: RwSignal::new(100),
            forward_distance: RwSignal::new(400),
        }
    }

    /// Generate timestamp from the date and time fields.
    pub(crate) fn get_timestamp_with_utc(&self) -> Timestamp {
        self.date.get().and_time(self.time.get()).and_utc()
    }
}
