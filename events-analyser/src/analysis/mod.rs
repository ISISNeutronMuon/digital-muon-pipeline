//! Defines the data type used in [FrameCache].
//!
//! [FrameCache]: crate::frame::FrameCache

mod with_true;

use std::collections::HashMap;

use digital_muon_common::Channel;
use tracing::info;

use crate::{engine::{AnalysisSettings, FlatBucketBlock}, event::ChannelData, eventlists::EventlistsCollection};

#[derive(Default)]
struct SumWithSumOfSqrs {
    sum: f64,
    sqr_sum: f64
}

impl SumWithSumOfSqrs {
    fn add_to(&mut self, value: f64) {
        self.sum += value;
        self.sqr_sum += value*value;
    }

    fn mean_and_stddev(&self, n: f64) -> (f64, f64) {
        (self.sum/n, f64::sqrt((n*self.sqr_sum - self.sum*self.sum)/(n*(n - 1.0))))
    }
}

#[derive(Default)]
pub(crate) struct PartialChannelAnalysis {
    channel: Channel,
    num_frames: usize,
    num_false_positives: SumWithSumOfSqrs,
    num_false_negatives: SumWithSumOfSqrs,
}

impl PartialChannelAnalysis {
    pub(crate) fn push(&mut self, mode: &AnalysisMode, mut collection: Vec<ChannelData>) {
        match mode {
            AnalysisMode::WithTrue { true_topic_index } => {
                let true_data = collection.remove(*true_topic_index);
                let estimate_data = collection;
            },
            AnalysisMode::ParallelEstimates {  } => {},
        }
    }
}

#[derive(Default)]
pub(crate) struct Analysis {
    channel: HashMap<Channel, PartialChannelAnalysis>,
}

impl Analysis {
    pub(crate) fn push(&mut self, mode: &AnalysisMode, collection: EventlistsCollection) {
        for (channel, channel_data) in collection.into_channel_collection() {
            self.channel.entry(channel).or_default().push(mode, channel_data);
        }
    }
}

pub(crate) enum AnalysisMode {
    WithTrue {
        true_topic_index: usize,
    },
    ParallelEstimates {

    }
}

impl AnalysisMode {
    pub(crate) fn new_with_true(true_topic_index: usize) -> Self {
        AnalysisMode::WithTrue { true_topic_index }
    }
}

enum MetricData {
    FalsePositive {
        num_traces: usize,
        sum: SumWithSumOfSqrs,
    },
    FalseNegatives {
        num_traces: usize,
        sum: SumWithSumOfSqrs,
    }
}

struct ChartData {
    /// metrics[metric][bucket]
    metrics: Vec<Vec<MetricData>>,
}

pub(crate) struct AnalysisEngine {
    settings: AnalysisSettings,
    analyses: Vec<Analysis>,
    buckets: Vec<FlatBucketBlock>
}

impl AnalysisEngine {
    pub(crate) fn new(settings: AnalysisSettings) -> Self {
        let buckets = settings.flatten_buckets().unwrap();
        info!("{buckets:?}");
        Self {
            settings,
            analyses: vec![ Analysis::default() ],
            buckets
        }
    }

    pub(crate) fn push(&mut self, collection: EventlistsCollection) {
        let bucket = self.buckets.iter().find_map(|block|block.buckets.iter().find(|bucket|bucket.is_collection_in(&collection)));
        if let Some(bucket) = bucket {
            info!("Success {:?}", bucket.criteria);
        } else {
            info!("Failure {:?}", collection.metadata);
        }
        //self.analyses[0].push(&self.settings, collection);
    }
}