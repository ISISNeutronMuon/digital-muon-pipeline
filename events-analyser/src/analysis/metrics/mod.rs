mod false_counts;
mod muon_lifetime;
mod result;

use crate::{
    engine::{FlatAlgorithm, FlatWaveform},
    event::ChannelData,
};
use digital_muon_common::Channel;
use std::{collections::HashMap, fmt::Display, ops::{Add, Sub}};

pub(crate) use result::PatrtialMetricResult;

#[derive(Default, Debug, Clone)]
struct SumWithSumOfSqrs {
    sum: f64,
    sqr_sum: f64,
}

impl SumWithSumOfSqrs {
    fn add_to(&mut self, value: f64) {
        self.sum += value;
        self.sqr_sum += value * value;
    }

    pub(crate) fn mean_and_stddev(&self, n: f64) -> (f64, f64) {
        (
            self.sum / n,
            f64::sqrt((n * self.sqr_sum - self.sum * self.sum) / (n * (n - 1.0))),
        )
    }
}

pub(crate) trait MetricChannelResult: Clone {
    type Source;
    type Aggregrate: MetricAggregatedResult<Channel = Self>;

    fn make_default(source: Self::Source) -> Self;
    fn push(
        &mut self,
        waveform: &FlatWaveform,
        algorithm: &FlatAlgorithm,
        by_topic: &[ChannelData],
    );
}

pub(crate) trait MetricAggregatedResult: Clone {
    type Channel: MetricChannelResult<Aggregrate = Self>;

    fn aggregate(source: &HashMap<Channel, Self::Channel>) -> Self;
    fn get_property(&self, property: &str) -> Result<MetricOutput<f64>, String>;
}

pub(crate) enum MetricOutput<T> {
    Scalar(T),
    ScalarWithBand(T, T),
}

impl<T : Copy> MetricOutput<Vec<T>> {
    fn append(&mut self, value: &MetricOutput<T>) {
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
    fn to_vector(&self, capacity: usize) -> MetricOutput<Vec<T>> {
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