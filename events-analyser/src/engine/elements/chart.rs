use crate::{
    analysis::MetricResult,
    engine::{
        AnalysisSettings, FlatBucketBlock, Flattenable, FlattenableWithIndex,
        elements::{MetricError, MetricProperty},
        values::{Dependency, ValueError},
    },
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub(crate) enum SeriesError {
    #[error("Block {0} has {1} buckets, and should have {2}.")]
    BucketInconsistancy(String, usize, usize),
    #[error("Bucket block not found, {0}.")]
    BucketNotFound(String),
    #[error("Metric not found, {0}.")]
    MetricNotFound(String),
    #[error("{0}.")]
    Metric(#[from] MetricError),
}

/// Encapsulates a series of data-points of a chart.
/// To specify the values used, the following must be specified:
/// a metric instance, a property of that metric, a bucket block instance.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Series {
    /// Name of series that appears in the key.
    name: String,
    /// Colour to apply to the line and marker on the graph.
    colour: Option<String>,
    /// Metric instance from which the y-values are collected.
    metric: String,
    /// Specific property of the metric from which the y-values are collected.
    property: String,
    /// Bucket block from which the y-values are collected.
    from_bucket: String,
}

impl Flattenable<&AnalysisSettings> for Series {
    type Flat = FlatSeries;
    type Error = SeriesError;

    fn flatten(&self, library: &AnalysisSettings) -> Result<Self::Flat, Self::Error> {
        let from_bucket = library
            .get_bucket_block_index(&self.from_bucket)
            .ok_or_else(|| SeriesError::BucketNotFound(self.from_bucket.clone()))?;

        let metric = library
            .get_metric_index(&self.metric)
            .ok_or_else(|| SeriesError::MetricNotFound(self.from_bucket.clone()))?;

        let property = library.get_property_of_metric(metric, &self.property)?;

        Ok(FlatSeries {
            name: self.name.clone(),
            colour: self.colour.clone(),
            from_bucket,
            metric,
            property,
        })
    }
}

/// Encapsulates a series of data-points of a chart, with all dependencies flattened.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatSeries {
    /// Name of series that appears in the key.
    pub(crate) name: String,
    /// Colour to apply to the line and marker on the graph.
    pub(crate) colour: Option<String>,
    /// Index of metric instance from which the y-values are collected.
    pub(crate) metric: usize,
    /// Specific property of the metric from which the y-values are collected.
    pub(crate) property: MetricProperty,
    /// Index of bucket block from which the y-values are collected.
    pub(crate) from_bucket: usize,
}

#[derive(Debug, Error)]
pub(crate) enum ChartError {
    #[error("Series Error: {0}")]
    Series(#[from] SeriesError),
    #[error("Value Error: {0}")]
    Value(#[from] ValueError),
    #[error("No output mode is set: set one of `output-to-json`, or `output-to-html` to `true`.")]
    NoOutputModeSet,
}

/// Defines a chart that can be written as a graphical chart, or as a json structure.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Chart {
    /// Number of values to use in the x-axis.
    width: usize,
    /// Values to display on the x-axis. This can either be from an array, or a function.
    x_axis: Dependency<f64>,
    #[serde(default)]
    /// Whether to write the chart to a json file (default: false).
    output_to_json: bool,
    #[serde(default)]
    /// Whether to write the chart to a graphical html file (default: false).
    output_to_html: bool,
    /// List of series to display on the graph.
    series: Vec<Series>,
    /// Label written on the x-axis.
    x_axis_label: String,
    /// Label written on the y-axis.
    y_axis_label: String,
    /// Title that appears on the graph (as well as the file name).
    title: String,
}

impl Flattenable<(&AnalysisSettings, &[FlatBucketBlock])> for Chart {
    type Flat = FlatChart;
    type Error = ChartError;

    fn flatten(
        &self,
        (library, buckets): (&AnalysisSettings, &[FlatBucketBlock]),
    ) -> Result<Self::Flat, Self::Error> {
        if self.output_to_html == false && self.output_to_json == false {
            return Err(ChartError::NoOutputModeSet);
        }

        let x_axis = (0..self.width)
            .map(|x| self.x_axis.flatten(library.templates.get_arrays(), x))
            .collect::<Result<Vec<_>, _>>()?;

        let series = self
            .series
            .iter()
            .map(|series| {
                series.flatten(library).and_then(|flat| {
                    let bucket_number = buckets
                        .get(flat.from_bucket)
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
                        Ok(flat)
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(FlatChart {
            ready: false,
            x_axis,
            series,
            output_to_html: self.output_to_html,
            output_to_json: self.output_to_json,
            x_axis_label: self.x_axis_label.clone(),
            y_axis_label: self.y_axis_label.clone(),
            title: self.title.clone(),
        })
    }
}

/// Defines a chart that can be written as a graphical chart, or as a json structure.
/// Flattened of all dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatChart {
    ready: bool,
    /// Whether to write the chart to a json file (default: false).
    pub(crate) output_to_json: bool,
    /// Whether to write the chart to a graphical html file (default: false).
    pub(crate) output_to_html: bool,
    /// Values to display on the x-axis. This can either be from an array, or a function.
    pub(crate) x_axis: Vec<f64>,
    /// List of series to display on the graph.
    pub(crate) series: Vec<FlatSeries>,
    /// Label written on the x-axis.
    pub(crate) x_axis_label: String,
    /// Label written on the y-axis.
    pub(crate) y_axis_label: String,
    /// Title that appears on the graph (as well as the file name).
    pub(crate) title: String,
}

impl FlatChart {
    /// Determines whether the chart is ready to be written.
    /// 
    /// # Parameters
    /// - buckets: 
    /// - metrics: 
    pub(crate) fn evaluate_readiness(&mut self, buckets: &[FlatBucketBlock], metrics: &[MetricResult]) -> bool {
        if self.ready {
            true
        } else {
            if self.is_chart_ready(buckets, metrics) {
                self.ready = true;
                true
            } else {
                false
            }
        }
    }

    /// Tests whether the chart is ready to be written.
    /// Namely whether all relevant metrics have enough data in their buckets.
    /// 
    /// # Parameters
    /// - buckets: 
    /// - metrics: 
    fn is_chart_ready(&self, buckets: &[FlatBucketBlock], metrics: &[MetricResult]) -> bool {
        for series in &self.series {
            let block = buckets
                .get(series.from_bucket)
                .expect("This should never fail");
            let metric = metrics.get(series.metric).expect("This should never fail");

            if !metric.are_buckets_full_enough(series.from_bucket, block.limits.min) {
                //info!("Testing Bucket Block: {}... block not ready.", block.name);
                return false;
            }
            info!("Testing Bucket Block: {}... block ready.", block.name);
        }
        true
    }
}
