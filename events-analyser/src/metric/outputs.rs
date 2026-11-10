pub(crate) enum PartialSeriesOutput {
    Value(Option<f64>),
    ValueWithErrors(Option<(f64, f64)>),
    LabelledGroup(Vec<(f64,String)>)
}

pub(crate) enum CompleteSeriesOutput {
    Value(Vec<Option<f64>>),
    ValueWithErrors(Vec<Option<(f64, f64)>>),
    LabelledGroup(Vec<Option<Vec<(f64,String)>>>),
}