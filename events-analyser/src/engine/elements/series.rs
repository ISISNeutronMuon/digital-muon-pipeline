use crate::engine::{AnalysisSettings, Flattenable, elements::metric::PropertyOfMetric};
use plotly::common::{DashType, Mode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum SeriesError {
    #[error("Block {0} has {1} buckets, and should have {2}.")]
    BucketInconsistancy(String, usize, usize),
    #[error("Bucket block not found, {0}.")]
    BucketNotFound(String),
    #[error("Metric not found, {0}.")]
    MetricNotFound(String),
}

/// Encapsulates settings of a series which do no need to be flattened.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SeriesBasicSettings {
    /// Name of series that appears in the key.
    pub(crate) name: String,
    /// Colour to apply to the line and marker on the graph.
    pub(crate) line_colour: Option<String>,
    /// Colour to apply to the line and marker on the graph.
    pub(crate) line_style: Option<DashStyle>,
    /// Whether to display the series as a bar chart (if applicable).
    pub(crate) series_type: SeriesType,
    /// If present, the title used to group series together.
    pub(crate) legend_group_title: Option<String>,
    /// The name of the x-axis to plot the series on.
    pub(crate) x_axis: String,
    /// The name of the y-axis to plot the series on.
    pub(crate) y_axis: String,
}

/// Encapsulates the style to use for the line of a series.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DashStyle {
    Solid,
    Dot,
    Dash,
    LongDash,
    DashDot,
    LongDashDot,
}

impl From<&DashStyle> for DashType {
    fn from(value: &DashStyle) -> Self {
        match value {
            DashStyle::Solid => DashType::Solid,
            DashStyle::Dot => DashType::Dot,
            DashStyle::Dash => DashType::Dash,
            DashStyle::LongDash => DashType::LongDash,
            DashStyle::DashDot => DashType::DashDot,
            DashStyle::LongDashDot => DashType::LongDashDot,
        }
    }
}

/// Encapsulates the style to use for the line of a series.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SeriesType {
    Scatter(ScatterType),
    Bar,
}

/// Encapsulates the style to use for the line of a series.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ScatterType {
    Marker,
    Line,
    MarkerWithLine,
}

impl From<&ScatterType> for Mode {
    fn from(value: &ScatterType) -> Self {
        match value {
            ScatterType::Marker => Mode::Markers,
            ScatterType::Line => Mode::Lines,
            ScatterType::MarkerWithLine => Mode::LinesMarkers,
        }
    }
}

/// Encapsulates a series of data-points of a chart.
/// To specify the values used, the following must be specified:
/// a metric instance, a property of that metric, a bucket block instance.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Series {
    #[serde(flatten)]
    settings: SeriesBasicSettings,
    /// Metric instance from which the y-values are collected.
    pub(crate) metric: String,
    /// Specific property of the metric from which the y-values are collected.
    property: PropertyOfMetric,
    /// Bucket block from which the y-values are collected.
    from_bucket: String,
}

/// Encapsulates a series of data-points of a chart, with all dependencies flattened.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatSeries {
    #[serde(flatten)]
    pub(crate) settings: SeriesBasicSettings,
    /// Index of metric instance from which the y-values are collected.
    pub(crate) metric: usize,
    /// Specific property of the metric from which the y-values are collected.
    pub(crate) property: PropertyOfMetric,
    /// Index of bucket block from which the y-values are collected.
    pub(crate) from_bucket_block: usize,
}

impl Flattenable<&AnalysisSettings> for Series {
    type Flat = FlatSeries;
    type Error = SeriesError;

    fn flatten(&self, library: &AnalysisSettings) -> Result<Self::Flat, Self::Error> {
        let from_bucket_block = library
            .get_bucket_block_index(&self.from_bucket)
            .ok_or_else(|| SeriesError::BucketNotFound(self.from_bucket.clone()))?;

        let metric = library
            .get_metric_index(&self.metric)
            .ok_or_else(|| SeriesError::MetricNotFound(self.metric.clone()))?;

        //let property = library.get_property_of_metric(metric, &self.property)?;

        Ok(FlatSeries {
            settings: self.settings.clone(),
            from_bucket_block,
            metric,
            property: self.property.clone(),
        })
    }
}
