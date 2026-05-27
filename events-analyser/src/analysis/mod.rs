//! Defines the data type used in [FrameCache].
//!
//! [FrameCache]: crate::frame::FrameCache
mod metrics;

use metrics::PatrtialMetricResult;
use crate::{engine::{AnalysisSettings, FlatChart, FlatBucketBlock, Flattenable}, eventlists::EventlistsCollection};

pub(crate) struct AnalysisEngine {
    buckets: Vec<FlatBucketBlock>,
    metrics: Vec<PatrtialMetricResult>,
    charts: Vec<FlatChart>,
}

impl AnalysisEngine {
    pub(crate) fn new(settings: AnalysisSettings) -> Result<Self,String> {
        let buckets = settings.flatten_buckets()
            .expect("This should never fail.");
        let bucket_block_sizes = buckets.iter()
            .map(|block|block.buckets.len())
            .collect::<Vec<_>>();
        
        let metrics = settings.metrics
            .iter()
            .map(|metric| metric
                .flatten(&settings.events_topics)
                .map(|metric|PatrtialMetricResult::new(metric, &bucket_block_sizes))
            )
            .collect::<Result<_,_>>()?;

        let charts = settings.charts
            .iter()
            .map(|chart|chart.flatten(&settings))
            .collect::<Result<Vec<_>,_>>()?;

        Ok(Self {
            metrics,
            buckets,
            charts,
        })
    }

    pub(crate) fn push<'a>(&'a mut self, collection: EventlistsCollection) -> Option<()> {
        let (index, bucket) = self.buckets
            .iter()
            .enumerate()
            .find_map(|(index, block)|
                block.find_bucket_matching(&collection)
                    .map(|(index_in_block, bucket)|((index, index_in_block),bucket))
            )?;
        
        let collection = collection.into_channel_collection();
        self.metrics
            .iter_mut()
            .for_each(|metric|
                metric.push(&bucket.waveform, &bucket.algorithm, index, &collection)
            );
        Some(())
    }

    pub(crate) fn build_charts(&self) {
        for chart in &self.charts {
            let from_buckets = chart.from_buckets
                .iter()
                .map(|bucket|self.buckets.get(*bucket).expect("This should never fail"));
            let metric = chart.metrics
                .iter()
                .map(|metric|self.metrics.get(*metric).expect("This should never fail"));
            let series = metric
                .flat_map(|metric|chart.from_buckets.iter().map(move |bucket|(metric,*bucket)))
                .collect::<Vec<_>>();

            for series in series {
                series.0;
            }
        }
    }
}
