use crate::{
    engine::{
        Flattenable, HasName,
        values::{Interval, ValueError},
    },
    impl_list_of_metrics,
    metric::{
        DependentMetricShell, ListOfMetrics, MetricWrapper, MetricWrapperWithTag,
        ResolvedMetricShell, dependent, properties, resolved,
    },
};
use serde::{Deserialize, Serialize};

/// Groups together all metric Json types.
#[derive(Debug, Clone)]
pub(crate) struct DependentMetricTypes;
impl_list_of_metrics!(DependentMetricTypes => DependentMetricShell<dependent>);

/// Defines a metric type, should only be created by deserializing JSON object.
pub(crate) type MetricType = MetricWrapperWithTag<DependentMetricTypes>;

/// Groups together all metric properties types.
#[derive(Debug, Clone)]
pub(crate) struct PropertyMetricTypes;

impl_list_of_metrics!(PropertyMetricTypes => properties);

/// Defines a property of a metric, should only be created from deserializing JSON file.
pub(crate) type PropertyOfMetric = MetricWrapper<PropertyMetricTypes>;

/// Groups together all flat metric types.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedMetricTypes;
impl_list_of_metrics!(ResolvedMetricTypes => ResolvedMetricShell<resolved>);

/// Resolved definition of a metric, should only be created by calling [DependentMetricShell::flatten].
pub(crate) type ResolvedMetricType = MetricWrapper<ResolvedMetricTypes>;

impl Flattenable<&[String]> for MetricType {
    type Flat = ResolvedMetricType;
    type Error = ValueError;

    fn flatten(&self, library: &[String]) -> Result<Self::Flat, Self::Error> {
        self.as_ref().try_map(
            |this| this.flatten(library),
            |this| this.flatten(library),
            |this| this.flatten(library),
            |this| this.flatten(library),
            |this| this.flatten(library),
            |this| this.flatten(library),
        )
    }
}

/// Defines a metric type, should only be created by deserializing JSON object.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Metric {
    /// Name of metric. This should be unique.
    pub(crate) name: String,
    /// Whether the metric's results should be saved in JSON format.
    #[serde(default)]
    pub(crate) save_to_json: bool,
    /// Defines the specific type of the metric.
    #[serde(flatten)]
    pub(crate) metric_type: MetricType,
}

impl HasName for Metric {
    fn get_name(&self) -> &str {
        &self.name
    }
}

/// Defines settings for a histogram type metric.
///
/// These do not require flattening.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct MetricTypeHistogram {
    /// Number of bins to use in the histogram.
    pub(crate) num_bins: usize,
    /// The upper and lower limits of the histogram.
    pub(crate) interval: Interval<f64>,
}

impl Flattenable<&[String]> for Metric {
    type Flat = FlatMetric;
    type Error = ValueError;

    fn flatten(&self, library: &[String]) -> Result<Self::Flat, Self::Error> {
        Ok(FlatMetric {
            name: self.get_name().to_string(),
            save_to_json: self.save_to_json,
            metric_type: self.metric_type.flatten(library)?,
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatMetric {
    pub(crate) name: String,
    pub(crate) save_to_json: bool,
    #[serde(flatten)]
    pub(crate) metric_type: ResolvedMetricType,
}

impl HasName for FlatMetric {
    fn get_name(&self) -> &str {
        &self.name
    }
}
