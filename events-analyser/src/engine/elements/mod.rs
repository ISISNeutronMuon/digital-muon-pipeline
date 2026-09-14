mod algorithm;
mod bucket;
mod chart;
mod criteria;
mod metric;
mod series;
mod waveform;

pub(crate) use {
    algorithm::{Algorithm, AlgorithmProperties, FlatAlgorithm},
    bucket::{
        BucketBlock, BucketBlockProperties, BucketBlockTemplate, BucketError, FlatBucket,
        FlatBucketBlock,
    },
    chart::{Chart, ChartError, FlatChart},
    criteria::CriteriaTemplate,
    metric::{
        EventCountProperty, FalseCountProperty, FlatMetric, FlatMetricEventCount,
        FlatMetricFalseCount, FlatMetricPulseHeightSpectra, FlatMetricMuonLifetime, FlatMetricType,
        PulseHeightSpectraProperty, Metric,
        MuonLifetimeProperty, PropertyOfMetric,
    },
    series::{FlatSeries, SeriesType},
    waveform::{FlatWaveform, Waveform, WaveformProperties},
};

#[cfg(test)]
pub(crate) use metric::MetricTypeHistogram;