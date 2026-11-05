use crate::{
    analysis::PartialMetricResult,
    engine::{
        AnalysisSettings, Chart, FlatBucketBlock, FlatChart, FlatSeries, Flattenable, FlattenableWithIndex, elements::{ChartError, series::{Series, SeriesError}}, values::{Dependency, ValueError}
    },
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum PageError {
    #[error("Chart Error: {0}")]
    Value(#[from] ChartError),
    #[error("No output mode is set: set one of `output-to-json`, or `output-to-html` to `true`.")]
    NoOutputModeSet,
}

/// Encapsulates settings of a chart which do no need to be flattened.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PageBasicSettings {
    #[serde(default)]
    /// Whether to write the chart to a json file (default: false).
    pub(crate) output_to_json: bool,
    #[serde(default)]
    /// Whether to write the chart to a graphical html file (default: false).
    pub(crate) output_to_html: bool,
    /// Title that appears on the page (as well as the file name).
    pub(crate) title: String,
}

/// Defines a chart that can be written as a graphical chart, or as a json structure.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Page {
    #[serde(flatten)]
    settings: PageBasicSettings,
    /// List of series to display on the graph.
    charts: Vec<Chart>,
}

/// Defines a chart that can be written as a graphical chart, or as a json structure.
/// Flattened of all dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatPage {
    #[serde(flatten)]
    pub(crate) settings: PageBasicSettings,
    /// List of series to display on the graph.
    pub(crate) charts: Vec<FlatChart>,
}

impl FlatPage {
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
        self.charts
            .iter_mut()
            .all(|chart|chart
                .evaluate_readiness(bucket_blocks, metrics)
            )
    }
}

impl Flattenable<(&AnalysisSettings, &[FlatBucketBlock])> for Page {
    type Flat = FlatPage;
    type Error = PageError;

    fn flatten(
        &self,
        (library, flat_bucket_blocks): (&AnalysisSettings, &[FlatBucketBlock]),
    ) -> Result<Self::Flat, Self::Error> {
        if !self.settings.output_to_html && !self.settings.output_to_json {
            return Err(PageError::NoOutputModeSet);
        }

        let charts = self
            .charts
            .iter()
            .map(|chart|chart.flatten((library, flat_bucket_blocks)))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(FlatPage {
            settings: self.settings.clone(),
            charts,
        })
    }
}
