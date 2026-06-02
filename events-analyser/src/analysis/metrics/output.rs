use std::{fmt::Display, ops::{Add, Sub}};

pub(crate) enum MetricOutput<T> {
    Scalar(T),
    ScalarWithBand(T, T),
}

impl<T : Copy> MetricOutput<Vec<T>> {
    pub(crate) fn append(&mut self, value: &MetricOutput<T>) {
        match (self, value) {
            (MetricOutput::Scalar(agg), MetricOutput::Scalar(val)) => agg.push(*val),
            (MetricOutput::ScalarWithBand(agg, agg_band), MetricOutput::ScalarWithBand(val, val_band)) => {
                agg.push(*val);
                agg_band.push(*val_band);
            },
            _ => unreachable!()
        }
    }
}

impl<T : Copy> MetricOutput<T> {
    pub(crate) fn to_vector(&self, capacity: usize) -> MetricOutput<Vec<T>> {
        match self {
            MetricOutput::Scalar(value) => MetricOutput::Scalar({
                let mut temp = Vec::with_capacity(capacity);
                temp.push(*value);
                temp
            }),
            MetricOutput::ScalarWithBand(value, band) => MetricOutput::ScalarWithBand(
                {
                    let mut temp = Vec::with_capacity(capacity);
                    temp.push(*value);
                    temp
                },
                {
                    let mut temp = Vec::with_capacity(capacity);
                    temp.push(*band);
                    temp
                },
            )
        }
    }
}

impl<T: ToString + Add<Output=T> + Sub<Output=T> + Copy> Display for MetricOutput<Vec<T>> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricOutput::Scalar(values) => {
                let string = values
                    .iter()
                    .map(|val| val.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                f.write_str(&string)
            }
            MetricOutput::ScalarWithBand(values, bands) => {
                let string = Iterator::zip(values.iter(), bands.iter())
                    .map(|(val, band)| (*val - *band).to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                f.write_str(&string)?;
                let string = Iterator::zip(values.iter(), bands.iter())
                    .map(|(val, band)| (*val + *band).to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                f.write_str(&string)
            }
        }
    }
}