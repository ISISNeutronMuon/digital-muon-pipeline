use crate::{
    analysis::metrics::{
        CompletedMetricResult, FittingError, HistogramWithBands, MetricOutputSeries,
        MetricResultError,
    },
    engine::{FlatChart, FlatSeries, SeriesType},
};
use plotly::{
    Bar, BoxPlot, Layout, Plot, Scatter, Trace,
    box_plot::{BoxMean, BoxPoints},
    common::{ErrorData, ErrorType, Line},
    layout::{Axis, ModeBar},
};
use serde::{Deserialize, Serialize};
use std::{fs::File, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ChartOutputError {
    #[error("Json Error: {0}")]
    Json(#[from] serde_json::error::Error),
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Metric Result Error: {0}")]
    Metric(#[from] MetricResultError),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ChartOutput {
    chart: FlatChart,
    data: Vec<Option<MetricOutputSeries>>,
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
                match metric
                    .get_aggregate_property(series.from_bucket_block, series.property.clone())
                {
                    Ok(value) => Ok(Some(value)),
                    Err(MetricResultError::Fitting(FittingError::NoValue)) => Ok(None),
                    Err(e) => Err(e),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
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
        path.push(&self.chart.settings.title);
        path.add_extension("json");
        let file = File::create(&path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }

    pub(crate) fn save_plotly(&self, path: &Path) -> Result<(), ChartOutputError> {
        let mut path = path.to_owned();
        path.push(&self.chart.settings.title);
        path.add_extension("html");
        let plot = self.build_graph();
        plot.write_html(&path);
        Ok(())
    }

    pub(crate) fn build_line(series: &FlatSeries) -> Line {
        let mut line = Line::new();
        if let Some(line_style) = &series.settings.line_style {
            line = line.dash(line_style.into());
        }
        if let Some(line_colour) = &series.settings.line_colour {
            line = line.color(line_colour.to_string());
        }
        line
    }

    fn build_scalar_x_axis<T>(&self, data: &[Option<T>]) -> Vec<f64> {
        self.chart
            .x_axis
            .iter()
            .zip(data)
            .filter_map(|(a, b)| b.is_some().then_some(*a))
            .collect::<Vec<_>>()
    }

    pub(crate) fn build_scalar_trace(
        &self,
        series: &FlatSeries,
        data: &[Option<f64>],
    ) -> Box<dyn Trace> {
        let x_axis = self.build_scalar_x_axis(data);
        let y_axis = data.iter().flatten().copied().collect::<Vec<_>>();
        match &series.settings.series_type {
            SeriesType::Scatter(scatter_type) => Scatter::new(x_axis, y_axis)
                .line(Self::build_line(series))
                .mode(scatter_type.into())
                .name(&series.settings.name),
            SeriesType::Bar => Bar::new(x_axis, y_axis).name(&series.settings.name),
        }
    }

    pub(crate) fn build_scalar_with_errors_trace(
        &self,
        series: &FlatSeries,
        data: &[Option<(f64, f64)>],
    ) -> Box<dyn Trace> {
        let x_axis = self.build_scalar_x_axis(data);
        let y_axis = data.iter().flatten().map(|x| x.0).collect::<Vec<_>>();
        let band = data.iter().flatten().map(|x| x.1).collect::<Vec<_>>();
        match &series.settings.series_type {
            SeriesType::Scatter(scatter_type) => Scatter::new(x_axis, y_axis)
                .line(Self::build_line(series))
                .name(&series.settings.name)
                .mode(scatter_type.into())
                .error_y(ErrorData::new(ErrorType::Data).array(band)),
            SeriesType::Bar => Bar::new(x_axis, y_axis)
                .error_y(ErrorData::new(ErrorType::Data).array(band))
                .name(&series.settings.name),
        }
    }

    pub(crate) fn build_group_trace(
        &self,
        series: &FlatSeries,
        data: &[Option<Vec<(f64, String)>>],
    ) -> Box<dyn Trace> {
        let x_axis = self
            .chart
            .x_axis
            .iter()
            .zip(data.iter())
            .filter_map(|(a, b)| b.as_ref().map(|b| vec![*a; b.len()]))
            .flatten()
            .collect::<Vec<_>>();
        let (y_axis, hover_text) = data
            .iter()
            .flatten()
            .flatten()
            .cloned()
            .unzip::<_, _, Vec<_>, Vec<_>>();

        BoxPlot::new_xy(x_axis, y_axis)
            .name(&series.settings.name)
            .box_points(BoxPoints::All)
            .jitter(10.0)
            .hover_text_array(hover_text)
            .box_mean(BoxMean::True)
    }

    pub(crate) fn build_histograms_trace(
        &self,
        series: &FlatSeries,
        data: &[HistogramWithBands],
    ) -> Vec<Box<dyn Trace>> {
        data.iter()
            .map(|histogram: &HistogramWithBands| {
                let mut error_y = ErrorData::new(ErrorType::Data).thickness(0.5);
                if let Some((upper, lower)) = histogram.bands.as_ref() {
                    error_y = error_y
                        .symmetric(false)
                        .array(upper.clone())
                        .array_minus(lower.clone())
                }
                let scatter = Scatter::new(histogram.labels.clone(), histogram.central.clone())
                    .line(Self::build_line(series))
                    .name(&series.settings.name)
                    .error_y(error_y);

                match &series.settings.series_type {
                    SeriesType::Scatter(scatter_type) => {
                        scatter.mode(scatter_type.into()) as Box<dyn Trace>
                    }
                    SeriesType::Bar => scatter as Box<dyn Trace>,
                }
            })
            .collect::<Vec<_>>()
    }

    pub(crate) fn build_trace(
        &self,
        series: &FlatSeries,
        data: Option<&MetricOutputSeries>,
    ) -> Vec<Box<dyn Trace>> {
        match data {
            Some(MetricOutputSeries::Value(data)) => vec![self.build_scalar_trace(series, data)],
            Some(MetricOutputSeries::WithErrors(data)) => {
                vec![self.build_scalar_with_errors_trace(series, data)]
            }
            Some(MetricOutputSeries::Group(data)) => vec![self.build_group_trace(series, data)],
            Some(MetricOutputSeries::Histograms(data)) => self.build_histograms_trace(series, data),
            None => vec![
                Scatter::<f64, f64>::new(Default::default(), Default::default())
                    .line(Self::build_line(series))
                    .name(format!("{} - values missing.", series.settings.name)),
            ],
        }
    }

    pub(crate) fn build_graph(&self) -> Plot {
        let mut plot: Plot = Plot::new();
        let layout = Layout::new()
            .title(&self.chart.settings.title)
            .mode_bar(ModeBar::new())
            .show_legend(true)
            .auto_size(true)
            .x_axis(Axis::new().title(&self.chart.settings.x_axis_label))
            .y_axis(Axis::new().title(&self.chart.settings.y_axis_label));

        plot.set_layout(layout);

        // Using `fold`` rather than `for` as this fixes some compiler type-checking issues.
        let add_traces = |mut plot: Plot, (series, series_data): (_, &Option<_>)| {
            let traces = self.build_trace(series, series_data.as_ref());
            plot.add_traces(traces);
            plot
        };
        Iterator::zip(self.chart.series.iter(), self.data.iter()).fold(plot, add_traces)
    }
}
