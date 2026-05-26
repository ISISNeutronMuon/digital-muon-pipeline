use crate::engine::values::Value;
use serde::Deserialize;

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
