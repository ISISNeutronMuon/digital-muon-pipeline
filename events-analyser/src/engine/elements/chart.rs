use crate::{
    analysis::PartialMetricResult,
    engine::{
        AnalysisSettings, FlatBucketBlock, FlatSeries, Flattenable, FlattenableWithIndex,
        elements::series::{Series, SeriesError},
        values::{Dependency, ValueError},
    },
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub(crate) enum ChartError {
    #[error("Series Error: {0}")]
    Series(#[from] SeriesError),
    #[error("Value Error: {0}")]
    Value(#[from] ValueError),
    #[error("No output mode is set: set one of `output-to-json`, or `output-to-html` to `true`.")]
    NoOutputModeSet,
}

/// Encapsulates settings of a chart which do no need to be flattened.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ChartBasicSettings {
    #[serde(default)]
    /// Whether to write the chart to a json file (default: false).
    pub(crate) output_to_json: bool,
    #[serde(default)]
    /// Whether to write the chart to a graphical html file (default: false).
    pub(crate) output_to_html: bool,
    /// Label written on the x-axis.
    pub(crate) x_axis_label: String,
    /// Label written on the y-axis.
    pub(crate) y_axis_label: String,
    /// Title that appears on the graph (as well as the file name).
    pub(crate) title: String,
    /// Number of rows the chart layout grid.
    pub(crate) num_rows: usize,
    /// Number of columns the chart layout grid.
    pub(crate) num_cols: usize,
    /// Number of columns the chart layout grid.
    pub(crate) width: Option<usize>,
    /// Width of each graph in this chart.
    pub(crate) height: Option<usize>,
    /// Horizontal space between graphs in this chart as a proportion of the width.
    pub(crate) x_gap_fraction: Option<f64>,
    /// Vertical space between graphs in this chart as a proportion of the height.
    pub(crate) y_gap_fraction: Option<f64>,
}

/// Defines a chart that can be written as a graphical chart, or as a json structure.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Chart {
    /// Number of values to use in the x-axis.
    width: usize,
    #[serde(flatten)]
    settings: ChartBasicSettings,
    /// Values to display on the x-axis. This can either be from an array, or a function.
    x_axis: Dependency<f64>,
    /// List of series to display on the graph.
    series: Vec<Series>,
}

/// Defines a chart that can be written as a graphical chart, or as a json structure.
/// Flattened of all dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatChart {
    ready: bool,
    #[serde(flatten)]
    pub(crate) settings: ChartBasicSettings,
    /// Values to display on the x-axis. This can either be from an array, or a function.
    pub(crate) x_axis: Vec<f64>,
    /// List of series to display on the graph.
    pub(crate) series: Vec<FlatSeries>,
}

impl FlatChart {
    /// Determines whether the chart is ready to be written.
    ///
    /// # Parameters
    /// - buckets_blocks: slice of all available bucket blocks.
    /// - metrics: slice of all available metrics.
    pub(crate) fn evaluate_readiness(
        &mut self,
        bucket_blocks: &[FlatBucketBlock],
        metrics: &[PartialMetricResult],
    ) -> bool {
        if self.ready {
            true
        } else if self.is_chart_ready(bucket_blocks, metrics) {
            self.ready = true;
            true
        } else {
            false
        }
    }

    /// Tests whether the chart is ready to be written.
    /// Namely whether all relevant metrics have enough data in their buckets.
    ///
    /// # Parameters
    /// - buckets_blocks: slice of all available bucket blocks.
    /// - metrics: slice of all available metrics.
    fn is_chart_ready(
        &self,
        buckets_blocks: &[FlatBucketBlock],
        metrics: &[PartialMetricResult],
    ) -> bool {
        for series in &self.series {
            let block = buckets_blocks
                .get(series.from_bucket_block)
                .expect("This should never fail");
            let metric = metrics.get(series.metric).expect("This should never fail");

            if !metric.are_buckets_full_enough(series.from_bucket_block, &block.buckets) {
                info!("Testing Bucket Block: {}... block not ready.", block.name);
                return false;
            }
            info!("Testing Bucket Block: {}... block ready.", block.name);
        }
        true
    }
}

impl Flattenable<(&AnalysisSettings, &[FlatBucketBlock])> for Chart {
    type Flat = FlatChart;
    type Error = ChartError;

    fn flatten(
        &self,
        (library, flat_bucket_blocks): (&AnalysisSettings, &[FlatBucketBlock]),
    ) -> Result<Self::Flat, Self::Error> {
        if !self.settings.output_to_html && !self.settings.output_to_json {
            return Err(ChartError::NoOutputModeSet);
        }

        let x_axis = (0..self.width)
            .map(|x| self.x_axis.flatten(library.templates.get_arrays(), x))
            .collect::<Result<Vec<_>, _>>()?;

        let series = self
            .series
            .iter()
            .map(|series| {
                series.flatten(library).and_then(|flat_series| {
                    let bucket_number = flat_bucket_blocks
                        .get(flat_series.from_bucket_block)
                        .expect("This should never fail.")
                        .buckets
                        .len();
                    if bucket_number != self.width {
                        Err(SeriesError::BucketInconsistancy(
                            series.metric.clone(),
                            bucket_number,
                            self.width,
                        ))
                    } else {
                        Ok(flat_series)
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(FlatChart {
            ready: false,
            settings: self.settings.clone(),
            x_axis,
            series,
        })
    }
}
