use crate::{
    analysis::metrics::{CompletedMetricResult, MetricOutput},
    engine::{FlatChart, FlatSeries},
};
use plotly::{
    self, Layout, Plot, Scatter,
    common::{ErrorData, ErrorType, Line},
    layout::{Axis, ModeBar},
};
use serde::{Deserialize, Serialize};
use std::{fs::File, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ChartOutputError {
    #[error("Json Error {0}")]
    Json(#[from] serde_json::error::Error),
    #[error("IO Error {0}")]
    IO(#[from] std::io::Error),
    #[error("Other Error {0}")]
    Other(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ChartOutput {
    chart: FlatChart,
    data: Vec<MetricOutput<Vec<f64>>>,
}

impl ChartOutput {
    pub(crate) fn new(
        chart: &FlatChart,
        metrics: &[CompletedMetricResult],
    ) -> Result<Self, ChartOutputError> {
        // Get Series Output
        let data = chart
            .series
            .iter()
            .map(|series: &FlatSeries| {
                let metric = metrics.get(series.metric).expect("This should never fail");
                metric.get_aggregate_property(series.from_bucket, &series.property)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(ChartOutputError::Other)?;
        Ok(Self {
            chart: chart.clone(),
            data,
        })
    }

    pub(crate) fn load_json(path: &Path, chart_name: &str) -> Result<Self, ChartOutputError> {
        let mut path = path.to_path_buf();
        path.push(chart_name);
        path.add_extension("json");
        Ok(serde_json::from_reader(File::open(&path)?)?)
    }

    pub(crate) fn save_json(&self, path: &Path) -> Result<(), ChartOutputError> {
        let mut path = path.to_owned();
        path.push(&self.chart.title);
        path.add_extension("json");
        let file = File::create(&path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }

    pub(crate) fn save_plotly(&self, path: &Path) -> Result<(), ChartOutputError> {
        let mut path = path.to_owned();
        path.push(&self.chart.title);
        path.add_extension("html");
        let plot = self.build_graph();
        plot.write_html(&path);
        Ok(())
    }

    pub(crate) fn build_graph(&self) -> Plot {
        let mut plot: Plot = Plot::new();
        let layout = Layout::new()
            .title(&self.chart.title)
            .mode_bar(ModeBar::new())
            .show_legend(true)
            .auto_size(true)
            .x_axis(Axis::new().title(&self.chart.x_axis_label))
            .y_axis(Axis::new().title(&self.chart.y_axis_label));

        plot.set_layout(layout);
        for (series, data) in Iterator::zip(self.chart.series.iter(), self.data.iter()) {
            match data {
                MetricOutput::Scalar(data) => {
                    let trace = Scatter::new(self.chart.x_axis.clone(), data.clone())
                        .line(Line::new())
                        .name(&series.name);
                    plot.add_trace(trace);
                }
                MetricOutput::ScalarWithBand(value, band) => {
                    let trace = Scatter::new(self.chart.x_axis.clone(), value.clone())
                        .line(Line::new())
                        .name(&series.name)
                        .error_y(ErrorData::new(ErrorType::Data).array(band.clone()));
                    plot.add_trace(trace);
                }
            }
        }
        plot
    }
}
