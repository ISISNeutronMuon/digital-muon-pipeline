use crate::engine::{
    AnalysisSettings, Flattenable, FlattenableWithIndex, Templates, values::Value,
};
use serde::Deserialize;
use tracing::error;

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Chart {
    from_buckets: Vec<String>,
    width: usize,
    x_axis: Value<f64>,
    metrics: Vec<String>,
    x_axis_label: String,
    title: String,
}

impl Flattenable for Chart {
    type Flat = FlatChart;
    type Library = AnalysisSettings;
    type Error = String;

    fn flatten(&self, library: &Self::Library) -> Result<Self::Flat, Self::Error> {
        let from_buckets = self
            .from_buckets
            .iter()
            .map(|bucket| {
                library.get_bucket_block_index_if(bucket, |bucket| bucket.number == self.width)
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| format!("Bucket block name not found"))?;

        let x_axis = (0..self.width)
            .map(|i| self.x_axis.flatten(library.templates.get_arrays(), i))
            .collect::<Result<Vec<_>, _>>()?;

        let metrics = self
            .metrics
            .iter()
            .map(|metric| library.get_metric_index(metric))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| format!("Metric name not found"))?;

        Ok(FlatChart {
            from_buckets,
            x_axis,
            metrics,
            x_axis_label: self.x_axis_label.clone(),
            title: self.title.clone(),
        })
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatChart {
    pub(crate) from_buckets: Vec<usize>,
    pub(crate) x_axis: Vec<f64>,
    pub(crate) metrics: Vec<usize>,
    pub(crate) x_axis_label: String,
    pub(crate) title: String,
}
