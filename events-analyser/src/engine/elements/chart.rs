use crate::{analysis::MetricResult, engine::{
    AnalysisSettings, FlatBucketBlock, Flattenable, FlattenableWithIndex, elements::{MetricError, MetricProperty}, values::{Value, ValueError}
}};
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

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Series {
    name: String,
    colour: Option<String>,
    metric: String,
    property: String,
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

        let property = library
            .get_property_of_metric(metric, &self.property)?;

        Ok(FlatSeries {
            name: self.name.clone(),
            colour: self.colour.clone(),
            from_bucket,
            metric,
            property,
        })
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatSeries {
    pub(crate) name: String,
    pub(crate) colour: Option<String>,
    pub(crate) metric: usize,
    pub(crate) property: MetricProperty,
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

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Chart {
    width: usize,
    x_axis: Value<f64>,
    #[serde(default)]
    output_to_json: bool,
    #[serde(default)]
    output_to_html: bool,
    series: Vec<Series>,
    x_axis_label: String,
    y_axis_label: String,
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

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatChart {
    ready: bool,
    pub(crate) output_to_json: bool,
    pub(crate) output_to_html: bool,
    pub(crate) x_axis: Vec<f64>,
    pub(crate) series: Vec<FlatSeries>,
    pub(crate) x_axis_label: String,
    pub(crate) y_axis_label: String,
    pub(crate) title: String,
}

impl FlatChart {
    pub(crate) fn poll(&self, buckets: &[FlatBucketBlock], metrics: &[MetricResult]) -> bool {
        if self.ready {
            return true;
        }
        for series in &self.series {
            let block = buckets
                .get(series.from_bucket)
                .expect("This should never fail");
            let metric = metrics
                .get(series.metric)
                .expect("This should never fail");

            if !metric.are_buckets_full_enough(series.from_bucket, block.limits.min) {
                //info!("Testing Bucket Block: {}... block not ready.", block.name);
                return false;
            }
            info!("Testing Bucket Block: {}... block ready.", block.name);
        }
        true
    }

    pub(crate) fn set_ready(&mut self) {
        self.ready = true;
    }
}
