use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MetricOutputGeneric<V, E, G, H> {
    Value(V),
    WithErrors(E),
    Group(G),
    Histograms(H),
}

impl<V1, E1, G1, H1> MetricOutputGeneric<V1, E1, G1, H1> {
    /// Matches the variants of `Self` with an instance of `MetricOutputGeneric` (with possibly different generic type arguments),
    /// and applies a closure to it, where the closure used depends on the variants and is given by the caller.
    /// Note that this method assumes both `self` and `other` use the same variant, and is not `Self::Empty`.
    pub(crate) fn apply_matched<V2, E2, G2, H2, FV, FE, FG, FH>(
        &mut self,
        other: &MetricOutputGeneric<V2, E2, G2, H2>,
        fv: FV,
        fe: FE,
        fg: FG,
        fh: FH,
    ) where
        FV: Fn(&mut V1, V2),
        FE: Fn(&mut E1, E2),
        FG: Fn(&mut G1, G2),
        FH: Fn(&mut H1, H2),
        V2: Clone,
        E2: Clone,
        G2: Clone,
        H2: Clone,
    {
        match (self, &other) {
            (Self::Value(vec), MetricOutputGeneric::Value(value)) => fv(vec, value.clone()),
            (Self::WithErrors(vec), MetricOutputGeneric::WithErrors(value)) => {
                fe(vec, value.clone())
            }
            (Self::Group(vec), MetricOutputGeneric::Group(value)) => fg(vec, value.clone()),
            (Self::Histograms(vec), MetricOutputGeneric::Histograms(value)) => {
                fh(vec, value.clone())
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct HistogramWithBands {
    pub(crate) labels: Vec<f64>,
    pub(crate) central: Vec<f64>,
    pub(crate) bands: Option<(Vec<f64>, Vec<f64>)>,
}
/*
impl HistogramWithBands {
    pub(crate) fn new(labels: &[f64]) -> Self {
        Self {
            num: 0.0,
            labels: labels.to_vec(),
            sum: vec![0.0; labels.len()],
            centre: vec![0.0; labels.len()],
            upper: vec![0.0; labels.len()],
            lower: vec![f64::MAX; labels.len()],
        }
    }

    pub(crate) fn append(mut self, histogram: &Histogram) -> Self {
        let zipped_iterators = self
            .centre
            .iter_mut()
            .zip(self.sum.iter_mut())
            .zip(Iterator::zip(self.upper.iter_mut(), self.lower.iter_mut()))
            .zip(histogram.get_counts().iter());
        for (((centre, sum), (upper, lower)), count) in zipped_iterators {
            *centre = (*centre * self.num + count) / (self.num + 1.0);
            *sum = *sum + count;
            *upper = upper.max(*count);
            *lower = lower.min(*count);
        }
        self.num += 1.0;
        self
    }
} */

/// Instance of `MetricOutputGeneric` which holds data derived from a single bucket.
pub(crate) type MetricOutput = MetricOutputGeneric<
    Option<f64>,
    Option<(f64, f64)>,
    Option<Vec<(f64, String)>>,
    HistogramWithBands,
>;

/// Instance of `MetricOutputGeneric` which holds data aggregated over several buckets.
///
/// To aggregate a collection of `MetricOutput` into a single `MetricOutputSeries` call the following:
/// ```rust
/// let metric_output_series = metric_output_collection.collect::<Option<MetricOutputSeries>>();
/// ```
/// The type should be collected into an `Option` wrapper, which is `None` if the `metric_output_collection` is empty.
pub(crate) type MetricOutputSeries = MetricOutputGeneric<
    Vec<Option<f64>>,
    Vec<Option<(f64, f64)>>,
    Vec<Option<Vec<(f64, String)>>>,
    Vec<HistogramWithBands>,
>;

impl FromIterator<MetricOutput> for Option<MetricOutputSeries> {
    fn from_iter<T: IntoIterator<Item = MetricOutput>>(iter: T) -> Self {
        iter.into_iter().fold(
            None,
            |series: Option<MetricOutputSeries>, value: MetricOutput| match series {
                Some(mut series) => {
                    series.apply_matched(&value, Vec::push, Vec::push, Vec::push, Vec::push);
                    Some(series)
                }
                None => Some(match value {
                    MetricOutput::Value(value) => MetricOutputSeries::Value(vec![value]),
                    MetricOutput::WithErrors(value) => MetricOutputSeries::WithErrors(vec![value]),
                    MetricOutput::Group(value) => MetricOutputSeries::Group(vec![value]),
                    MetricOutput::Histograms(value) => MetricOutputSeries::Histograms(vec![value]),
                }),
            },
        )
    }
}
