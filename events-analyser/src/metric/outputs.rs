use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum PartialSeriesOutput {
    Value(Option<f64>),
    ValueWithErrors(Option<(f64, f64)>),
    LabelledGroup(Vec<(f64,String)>)
}

impl FromIterator<PartialSeriesOutput> for Option<CompleteSeriesOutput> {
    fn from_iter<T: IntoIterator<Item = PartialSeriesOutput>>(iter: T) -> Self {
        iter.into_iter().fold(
            None,
            |series: Option<CompleteSeriesOutput>, value: PartialSeriesOutput| match series {
                Some(mut series) => {
                    match (&mut series, value) {
                        (CompleteSeriesOutput::Value(vec), PartialSeriesOutput::Value(value)) => vec.push(value),
                        (CompleteSeriesOutput::ValueWithErrors(vec), PartialSeriesOutput::ValueWithErrors(value)) => vec.push(value),
                        (CompleteSeriesOutput::LabelledGroup(vec), PartialSeriesOutput::LabelledGroup(value)) => vec.push(Some(value)),
                        _ => unreachable!()
                    }
                    Some(series)
                }
                None => Some(match value {
                    PartialSeriesOutput::Value(value) => CompleteSeriesOutput::Value(vec![value]),
                    PartialSeriesOutput::ValueWithErrors(value) => CompleteSeriesOutput::ValueWithErrors(vec![value]),
                    PartialSeriesOutput::LabelledGroup(value) => CompleteSeriesOutput::LabelledGroup(vec![Some(value)]),
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


#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum CompleteSeriesOutput {
    Value(Vec<Option<f64>>),
    ValueWithErrors(Vec<Option<(f64, f64)>>),
    LabelledGroup(Vec<Option<Vec<(f64,String)>>>),
    Histogram(HistogramWithBands),
    HeatMap(Surface)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct HistogramWithBands {
    pub(crate) labels: Vec<f64>,
    pub(crate) central: Vec<f64>,
    pub(crate) bands: Option<(Vec<f64>, Vec<f64>)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Surface {
    pub(crate) x_values: Vec<f64>,
    pub(crate) y_values: Vec<f64>,
    pub(crate) data: Vec<Vec<f64>>,
}
