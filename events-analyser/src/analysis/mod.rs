//! Defines the data type used in [FrameCache].
//!
//! [FrameCache]: crate::frame::FrameCache
mod chart;
mod metrics;

use crate::{
    analysis::chart::ChartOutputError, engine::{AnalysisSettings, FlatBucketBlock, FlatChart}, eventlists::EventlistsCollection
};
use digital_muon_common::{Channel, DigitizerId, spanned::{SpanOnceError, Spanned, SpannedAggregator}};
use digital_muon_streaming_types::FrameMetadata;
use std::{fs::File, path::{Path, PathBuf}};
use thiserror::Error;
use tracing::{info, info_span, trace};
pub(crate) use chart::ChartOutput;
pub(crate) use metrics::MetricResult;

#[derive(Debug, Error)]
pub(crate) enum AnalysisError {
    #[error("No bucket found matching eventlist criteria: {0}, {1:?}, {2:?}.")]
    NoBucketMatchesCriteria(DigitizerId, FrameMetadata, Vec<Channel>),
    #[error("Json Error {0}")]
    Json(#[from] serde_json::error::Error),
    #[error("IO Error {0}")]
    IO(#[from] std::io::Error),
    #[error("Chart Error: {0}")]
    Chart(#[from] ChartOutputError),
    #[error("Span Error: {0}")]
    Span(#[from] SpanOnceError),
    #[error("No Json Metric Specified")]
    NoJsonMetricSpecified
}

pub(crate) struct AnalysisEngine {
    path: PathBuf,
    metrics_json_name: Option<String>,
    buckets: Vec<FlatBucketBlock>,
    metrics: Vec<MetricResult>,
    charts: Vec<FlatChart>,
}

impl AnalysisEngine {
    pub(crate) fn new(settings: AnalysisSettings, path: PathBuf, load_metrics: bool) -> Result<Self, AnalysisError> {
        let buckets = settings.flatten_buckets().expect("Fixme: This may fail.");

        let bucket_block_sizes = buckets
            .iter()
            .map(|block| block.buckets.len())
            .collect::<Vec<_>>();

        let charts = settings
            .flatten_charts(&buckets)
            .expect("Fixme: This may fail.");

        let metrics = settings
            .flatten_metrics()
            .expect("Fixme: This may fail.")
            .into_iter()
            .map(|metric| MetricResult::new(metric, &bucket_block_sizes) )
            .collect::<Vec<_>>();

        let mut this = Self {
            path,
            metrics,
            buckets,
            charts,
            metrics_json_name: settings.metrics_json_name
        };
        if load_metrics {
            this.load_json_metrics()?;
        }
        Ok(this)
    }

    pub(crate) fn push(&mut self, collection: EventlistsCollection) -> Result<(), AnalysisError> {
        let (index, bucket) = self
            .buckets
            .iter_mut()
            .enumerate()
            .find_map(|(index, block)| {
                block
                    .find_bucket_matching(&collection)
                    .map(|(index_in_block, bucket)| ((index, index_in_block), bucket))
            })
            .ok_or_else(|| {
                AnalysisError::NoBucketMatchesCriteria(
                    collection.digitiser_id,
                    collection.metadata.clone(),
                    collection.channels.clone(),
                )
            })?;

        if let Some(bucket) = bucket {
            collection.span().get()
                .expect("This should never fail")
                .in_scope(||bucket.link_current_span(||info_span!("EventList")))
                .expect("This should never fail");

            bucket.increment_count();
            info!("Pushing to bucket {}, {}. Count: {}", index.0, index.1, bucket.count);
            let collection = collection.into_channel_collection();
            self.metrics.iter_mut().for_each(|metric| {
                metric.push(&bucket.waveform, &bucket.algorithm, index, &collection)
            });
        } else {
            info!("Bucket {}, {} full", index.0, index.1);
        }
        Ok(())
    }

    pub(crate) fn load_json_metrics(&mut self) -> Result<(), AnalysisError> {
        if let Some(metrics_json_name) = &self.metrics_json_name {
            let mut path = self.path.clone();
            path.push(metrics_json_name);
            path.add_extension("json");
            self.metrics = serde_json::from_reader(File::open(&path)?)?;
            Ok(())
        } else {
            Err(AnalysisError::NoJsonMetricSpecified)
        }
    }

    pub(crate) fn save_metrics_json(&self, path: &Path, metrics_json_name: &str) -> Result<(), AnalysisError> {
        let mut path = path.to_owned();
        path.push(metrics_json_name);
        path.add_extension("json");
        let file = File::create(&path)?;
        serde_json::to_writer_pretty(file, &self.metrics)?;
        Ok(())
    }

    pub(crate) fn build_charts(&mut self) -> Result<(), AnalysisError> {
        if let Some(metrics_json_name) = &self.metrics_json_name {
            self.save_metrics_json(&self.path, metrics_json_name)?;
        }
        for chart in &self.charts {
            // Ensure all series' metrics have been aggregated.
            for series in &chart.series {
                self.metrics
                    .get_mut(series.metric)
                    .expect("This should never fail")
                    .build_aggregate();
            }
            
            let output = ChartOutput::new(chart, &self.metrics)?;
            if chart.output_to_json {
                output.save_json(&self.path)?;
            }
            if chart.output_to_html {
                output.save_plotly(&self.path)?;
            }
        }
        Ok(())
    }

    pub(crate) fn chart_poll(&mut self) -> Result<bool, String> {
        for chart in &mut self.charts {
            if chart.poll(&self.buckets, &self.metrics) {
                chart.set_ready();
                trace!("{}, ready.", chart.title);
            } else {
                trace!("{}, not ready.", chart.title);
                return Ok(false);
            }
        }
        Ok(true)
    }
}
