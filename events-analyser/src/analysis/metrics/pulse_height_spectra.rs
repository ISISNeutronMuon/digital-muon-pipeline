use crate::{
    analysis::metrics::{
        MetricOutput, MetricResultError,
        output::HistogramWithBands,
        results::{CompleteMetricResultClass, PartialMetricResultClass},
        utils::Histogram,
    },
    engine::{
        FlatAlgorithm, FlatMetricPulseHeightSpectra, FlatWaveform, Interval,
        PulseHeightSpectraProperty,
    },
    eventlists::ChannelDataByTopic,
};
use digital_muon_common::Channel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PartialPulseHeightSpectra {
    num: usize,
    num_bins: usize,
    interval: Interval<f64>,
    topic: usize,
    histogram: HashMap<Channel, Histogram>,
}

impl PartialMetricResultClass for PartialPulseHeightSpectra {
    type Source = FlatMetricPulseHeightSpectra;
    type Complete = CompletedPulseHeightSpectra;

    fn make_default(source: &FlatMetricPulseHeightSpectra) -> Self {
        Self {
            num: Default::default(),
            topic: source.topic,
            num_bins: source.histogram.num_bins,
            interval: source.histogram.interval.clone(),
            histogram: Default::default(),
        }
    }

    fn load_data(&mut self, source: &Self) {
        self.num = source.num;
        self.topic = source.topic;
        self.histogram = source.histogram.clone();
    }

    fn push(
        &mut self,
        _waveform: &FlatWaveform,
        _algorithm: &FlatAlgorithm,
        channel: Channel,
        by_topic: &ChannelDataByTopic,
    ) {
        self.num += 1;
        for (_, intensity) in by_topic
            .get(self.topic)
            .expect("This should never fail.")
            .get_time_intensity()
        {
            self.histogram
                .entry(channel)
                .or_insert(Histogram::new(self.num_bins, &self.interval))
                .push(*intensity as f64);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CompletedPulseHeightSpectra {
    histograms: HashMap<Channel, Histogram>,
}

impl CompleteMetricResultClass for CompletedPulseHeightSpectra {
    type Partial = PartialPulseHeightSpectra;
    type Error = MetricResultError;
    type Property = PulseHeightSpectraProperty;

    fn aggregate(source: &Self::Partial) -> Result<Self, Self::Error> {
        Ok(CompletedPulseHeightSpectra {
            histograms: source.histogram.clone(),
        })
    }

    fn get_property(&self, property: Self::Property) -> Result<MetricOutput, Self::Error> {
        match property {
            PulseHeightSpectraProperty::Histograms => {
                let bin_labels = self
                    .histograms
                    .values()
                    .next()
                    .expect("No histogram values, this should never happen.")
                    .get_bin_labels(); // FIXME: This might happen.
                let histogram = self.histograms.values().fold(
                    HistogramWithBands::new(bin_labels),
                    HistogramWithBands::append,
                );
                Ok(MetricOutput::Histograms(histogram))
            }
        }
    }
}
