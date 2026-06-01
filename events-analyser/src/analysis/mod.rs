//! Defines the data type used in [FrameCache].
//!
//! [FrameCache]: crate::frame::FrameCache
mod metrics;

use crate::{
    analysis::metrics::MetricOutput,
    engine::{AnalysisSettings, FlatBucketBlock, FlatChart, WithName},
    eventlists::EventlistsCollection,
};
use digital_muon_common::{Channel, DigitizerId};
use digital_muon_streaming_types::FrameMetadata;
use metrics::PatrtialMetricResult;
use std::{fs::File, io::Write, path::PathBuf};
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub(crate) enum AnalysisError {
    #[error("No bucket found matching eventlist criteria: {0}, {1:?}, {2:?}.")]
    NoBucketMatchesCriteria(DigitizerId, FrameMetadata, Vec<Channel>),
}

pub(crate) struct AnalysisEngine {
    path: PathBuf,
    buckets: Vec<WithName<FlatBucketBlock>>,
    metrics: Vec<PatrtialMetricResult>,
    charts: Vec<FlatChart>,
}

impl AnalysisEngine {
    pub(crate) fn new(settings: AnalysisSettings, path: PathBuf) -> Result<Self, String> {
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
            .map(|metric| PatrtialMetricResult::new(metric, &bucket_block_sizes))
            .collect::<Vec<_>>();

        Ok(Self {
            path,
            metrics,
            buckets,
            charts,
        })
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
            bucket.increment_count();

            info!("Pushing to bucket {}, {}.", index.0, index.1);
            let collection = collection.into_channel_collection();
            self.metrics.iter_mut().for_each(|metric| {
                metric.push(&bucket.waveform, &bucket.algorithm, index, &collection)
            });
        }
        Ok(())
    }

    pub(crate) fn build_charts(&self) -> Result<(), String> {
        for chart in &self.charts {
            if chart.is_built() {
                continue;
            }
            let mut path = self.path.clone();
            path.push(&chart.title);
            let mut file = File::create(path).unwrap();
            let series = chart
                .series
                .iter()
                .map(|series| {
                    let metric = self
                        .metrics
                        .get(series.metric)
                        .expect("This should never fail");
                    metric.get_aggregate_property(series.from_bucket, &series.property)
                })
                .collect::<Result<Vec<_>, _>>()?;

            for series in series {
                match series {
                    MetricOutput::Scalar(values) => {
                        let string = values
                            .iter()
                            .map(|val| val.to_string())
                            .collect::<Vec<_>>()
                            .join(",");
                        writeln!(&mut file, "{string}").unwrap();
                    }
                    MetricOutput::ScalarWithBand(values, bands) => {
                        let string = values
                            .iter()
                            .map(|val| val.to_string())
                            .collect::<Vec<_>>()
                            .join(",");
                        writeln!(&mut file, "{string}").unwrap();
                        let string = Iterator::zip(values.iter(), bands.iter())
                            .map(|(val, band)| (val - band).to_string())
                            .collect::<Vec<_>>()
                            .join(",");
                        writeln!(&mut file, "{string}").unwrap();
                        let string = Iterator::zip(values.iter(), bands.iter())
                            .map(|(val, band)| (val + band).to_string())
                            .collect::<Vec<_>>()
                            .join(",");
                        writeln!(&mut file, "{string}").unwrap();
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn chart_poll(&mut self) -> Result<bool, String> {
        for chart in &self.charts {
            if chart.poll(&self.buckets) {
                info!("{}, complete.", chart.title);
            } else {
                info!("{}, pending.", chart.title);
                return Ok(false);
            }
        }
        for chart in &mut self.charts {
            if chart.is_built() {
                continue;
            }
            for series in &mut chart.series {
                let metric = self
                    .metrics
                    .get_mut(series.metric)
                    .expect("This should never fail");
                metric.build_aggregate();
            }
        }

        self.build_charts()?;
        Ok(true)
    }
}
