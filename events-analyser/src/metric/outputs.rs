use serde::{Deserialize, Serialize};

pub(crate) type LabelledGroupData = Vec<(f64, String)>;

/// Contains the output of a single metric bucket.
/// Obtained by calling [CompleteMetric::get_property].
/// This type will usually be part of a sequence obtained from buckets within a block.
/// Before being used by a [Series] it should be concatenated into a [CompleteSeriesOutput] object
/// by calling [complete] on an iterator over [PartialSeriesOutput].
///
/// # Example
///
/// ```rust
/// let mut list : Vec<PartialSeriesOutput> = Vec::new();
/// // Add some `PartialSeriesOutput` to list
/// let output : CompleteSeriesOutput = list.into_iter().complete();
/// ```
///
/// [complete]: [std::iter::Iterator::<Item = PartialSeriesOutput>::complete]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum PartialSeriesOutput {
    /// A scalar value.
    Value(Option<f64>),
    /// A scalar value along with an error.
    ValueWithErrors(Option<(f64, f64)>),
    /// A list of scalar values, each with a label.
    LabelledGroup(LabelledGroupData),
}

impl FromIterator<PartialSeriesOutput> for Option<CompleteSeriesOutput> {
    fn from_iter<T: IntoIterator<Item = PartialSeriesOutput>>(iter: T) -> Self {
        iter.into_iter().fold(
            None,
            |series: Option<CompleteSeriesOutput>, value: PartialSeriesOutput| match series {
                Some(mut series) => {
                    match (&mut series, value) {
                        (CompleteSeriesOutput::Value(vec), PartialSeriesOutput::Value(value)) => {
                            vec.push(value)
                        }
                        (
                            CompleteSeriesOutput::ValueWithErrors(vec),
                            PartialSeriesOutput::ValueWithErrors(value),
                        ) => vec.push(value),
                        (
                            CompleteSeriesOutput::LabelledGroup(vec),
                            PartialSeriesOutput::LabelledGroup(value),
                        ) => vec.push(Some(value)),
                        _ => unreachable!(),
                    }
                    Some(series)
                }
                None => Some(match value {
                    PartialSeriesOutput::Value(value) => CompleteSeriesOutput::Value(vec![value]),
                    PartialSeriesOutput::ValueWithErrors(value) => {
                        CompleteSeriesOutput::ValueWithErrors(vec![value])
                    }
                    PartialSeriesOutput::LabelledGroup(value) => {
                        CompleteSeriesOutput::LabelledGroup(vec![Some(value)])
                    }
                }),
            },
        )
    }
}

impl FromIterator<CompleteSeriesOutput> for Option<CompleteSeriesOutput> {
    fn from_iter<T: IntoIterator<Item = CompleteSeriesOutput>>(iter: T) -> Self {
        iter.into_iter().fold(
            None,
            |series: Option<CompleteSeriesOutput>, value: CompleteSeriesOutput| match series {
                Some(_series) => unreachable!(),
                None => Some(value),
            },
        )
    }
}

/// Contains the output of a metric bucket block, concatenated from all buckets in the block.
/// Obtained by calling [MetricResultByBucket::get_property].
///
/// Note that [CompleteSeriesOutput] is obtained by calling [complete] on an iterator over a sequence of [PartialSeriesOutput],
/// or trivially over an iterator on a single [CompleteSeriesOutput].
///
/// Note that, in the second case the component panics if the iterator is over more than one item.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum CompleteSeriesOutput {
    /// A list of scalar values.
    Value(Vec<Option<f64>>),
    /// A list of scalar values each with an error.
    ValueWithErrors(Vec<Option<(f64, f64)>>),
    /// A list of lists of scalar values, each with a label.
    LabelledGroup(Vec<Option<LabelledGroupData>>),
    /// A histogram
    Histogram(HistogramWithBands),
    /// A 2D surface representing values.
    HeatMap(Surface),
}

/// Encapsulates the data from a histogram with optional error bands.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct HistogramWithBands {
    /// X axis labels of the histogram.
    pub(crate) labels: Vec<f64>,
    /// Values of the histogram.
    pub(crate) central: Vec<f64>,
    /// If present, the upper and lower
    pub(crate) bands: Option<(Vec<f64>, Vec<f64>)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Surface {
    pub(crate) x_values: Vec<f64>,
    pub(crate) y_values: Vec<f64>,
    pub(crate) data: Vec<Vec<f64>>,
}
