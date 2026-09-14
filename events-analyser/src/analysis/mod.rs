//! Defines the data type used in [FrameCache].
//!
//! [FrameCache]: crate::frame::FrameCache
mod chart;
mod metrics;

use crate::{
    analysis::{chart::ChartOutputError, metrics::MetricResultError},
    engine::{AnalysisSettings, FlatBucketBlock, FlatChart},
    eventlists::EventlistsCollection,
};
use chrono::{DateTime, ParseError, TimeDelta, Utc};
use digital_muon_common::{
    Channel, DigitizerId,
    spanned::{SpanOnceError, Spanned, SpannedAggregator},
};
use digital_muon_streaming_types::FrameMetadata;
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use thiserror::Error;
use tracing::{info, info_span, trace};

pub(crate) use chart::ChartOutput;
pub(crate) use metrics::PartialMetricResult;

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
    #[error("Metric Result Error: {0}")]
    MetricResult(#[from] MetricResultError),
    #[error("No Json Metric Specified")]
    NoJsonMetricSpecified,
    #[error("DateTime parse error {0}")]
    ParseError(#[from] ParseError),
}

#[derive(Clone, Copy)]
pub(crate) struct BucketIndex {
    pub(crate) block_index: usize,
    pub(crate) bucket_index: usize,
}

pub(crate) struct AnalysisEngine {
    path: PathBuf,
    metrics_json_name: Option<String>,
    buckets_blocks: Vec<FlatBucketBlock>,
    metrics: Vec<PartialMetricResult>,
    charts: Vec<FlatChart>,
    idle_time: TimeDelta,
    last_message_timestamp: Option<DateTime<Utc>>,
}

impl AnalysisEngine {
    pub(crate) fn new(
        settings: AnalysisSettings,
        path: PathBuf,
        load_metrics: bool,
    ) -> Result<Self, AnalysisError> {
        let buckets_blocks = settings
            .flatten_bucket_blocks()
            .expect("Fixme: This may fail.");

        let bucket_block_sizes = buckets_blocks
            .iter()
            .map(|block| block.buckets.len())
            .collect::<Vec<_>>();

        let charts = settings
            .flatten_charts(&buckets_blocks)
            .expect("Fixme: This may fail.");

        let metrics = settings
            .flatten_metrics()
            .expect("Fixme: This may fail.")
            .into_iter()
            .map(|metric| PartialMetricResult::new(metric.metric_type, &bucket_block_sizes))
            .collect::<Vec<_>>();

        info!(
            "Analysis Engine created with {} metric(s), {} chart(s), and {} bucket block(s).",
            metrics.len(),
            charts.len(),
            bucket_block_sizes.len()
        );
        info!("Metric(s): {metrics:?}");
        info!("Chart(s): {charts:?}");
        info!("Bucket size(s): {bucket_block_sizes:?}");

        let mut this = Self {
            path,
            metrics,
            buckets_blocks,
            charts,
            metrics_json_name: settings.metrics_json_name,
            idle_time: TimeDelta::seconds(settings.idle_time_sec),
            last_message_timestamp: None,
        };
        if load_metrics {
            this.load_json_metrics()?;
        }
        Ok(this)
    }

    pub(crate) fn push(&mut self, collection: EventlistsCollection) -> Result<(), AnalysisError> {
        let (index, bucket) = self
            .buckets_blocks
            .iter_mut()
            .enumerate()
            .find_map(|(block_index, block)| {
                block
                    .find_bucket_matching(&collection)
                    .map(|(bucket_index, bucket)| {
                        (
                            BucketIndex {
                                block_index,
                                bucket_index,
                            },
                            bucket,
                        )
                    })
            })
            .ok_or_else(|| {
                AnalysisError::NoBucketMatchesCriteria(
                    collection.digitiser_id,
                    collection.metadata.clone(),
                    collection.channels.clone(),
                )
            })?;

        if let Some(bucket) = bucket {
            collection
                .span()
                .get()
                .expect("This should never fail")
                .in_scope(|| bucket.link_current_span(|| info_span!("EventList")))
                .expect("This should never fail");

            bucket.increment_count();
            info!(
                "Pushing to bucket {}, {}. Count: {}",
                index.block_index, index.bucket_index, bucket.count
            );
            let collection = collection.into_channel_collection();
            self.metrics.iter_mut().for_each(|metric| {
                metric.push(&bucket.waveform, &bucket.algorithm, index, &collection)
            });
        } else {
            info!("Bucket {}, {} full", index.block_index, index.bucket_index);
        }

        // Update last idle time field.
        self.last_message_timestamp = Some(Utc::now());
        Ok(())
    }

    pub(crate) fn load_json_metrics(&mut self) -> Result<(), AnalysisError> {
        if let Some(metrics_json_name) = &self.metrics_json_name {
            let mut path = self.path.clone();
            path.push(metrics_json_name);
            path.add_extension("json");

            let metrics_from_file: Vec<PartialMetricResult> =
                serde_json::from_reader(File::open(&path)?)?;
            for (old, new) in Iterator::zip(self.metrics.iter_mut(), metrics_from_file.iter()) {
                old.load_data(new)?;
            }

            // Set the last message timestamp field to trigger the evaluation.
            self.last_message_timestamp = Some(
                Utc::now()
                    .checked_sub_signed(self.idle_time)
                    .expect("Subtracted time should be in range, this should never fail."),
            );
            Ok(())
        } else {
            Err(AnalysisError::NoJsonMetricSpecified)
        }
    }

    pub(crate) fn save_metrics_json(
        &self,
        path: &Path,
        metrics_json_name: &str,
    ) -> Result<(), AnalysisError> {
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
        let metrics = self
            .metrics
            .iter()
            .map(|part| part.build_aggregate())
            .collect::<Result<Vec<_>, _>>()?;

        for chart in &self.charts {
            let output = ChartOutput::new(chart, &metrics)?;

            if chart.settings.output_to_json {
                output.save_json(&self.path)?;
            }
            if chart.settings.output_to_html {
                output.save_plotly(&self.path)?;
            }
        }
        Ok(())
    }

    /// Determine whether the charts are ready to be created.
    /// That is, return whether the idle time condition has passed, and
    /// whether each applicable metric has collected enough data.
    pub(crate) fn evaluate_chart_readiness(&mut self) -> bool {
        if let Some(last_message_timestamp) = self.last_message_timestamp
            && Utc::now() - last_message_timestamp > self.idle_time
        {
            info!(
                "Idle Time Trigger Satisfied, last push: {}.",
                last_message_timestamp.to_rfc3339()
            );
            for chart in &mut self.charts {
                if chart.evaluate_readiness(&self.buckets_blocks, &self.metrics) {
                    trace!("{}, ready.", chart.settings.title);
                } else {
                    trace!("{}, not ready.", chart.settings.title);
                    return false;
                }
            }
            true
        } else {
            false
        }
    }
}
