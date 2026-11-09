use crate::{
    analysis::{
        chart_page::TraceExt,
        metrics::{
            CompletedMetricResult, FittingError, HistogramWithBands, MetricOutputSeries,
            MetricResultError, Surface,
        },
    },
    engine::{FlatChart, FlatSeries, SeriesType},
};
use num::Integer;
use plotly::{
    Bar, BoxPlot, HeatMap, Layout, Plot, Scatter, Trace,
    box_plot::{BoxMean, BoxPoints},
    common::{ErrorData, ErrorType, LegendGroupTitle, Line},
    heat_map::Smoothing,
    layout::{
        Axis, GridPattern, GroupClick, ItemClick, LayoutGrid, Legend, ModeBar, TraceOrder, VAlign,
    },
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

impl<X, Y> TraceExt for Box<Scatter<X, Y>>
where
    X: Clone + Serialize,
    Y: Clone + Serialize,
{
    fn apply_settings(self, series: &FlatSeries) -> Self {
        let mut line = Line::new();
        if let Some(line_style) = &series.settings.line.line_style {
            line = line.dash(line_style.into());
        }
        if let Some(line_colour) = &series.settings.line.line_colour {
            line = line.color(line_colour.to_string());
        }

        self.name(&series.settings.name)
            .line(line)
            .x_axis(&series.settings.axis.x_axis)
            .y_axis(&series.settings.axis.y_axis)
            .legend_group(format!(
                "x{0}y{1}",
                series.settings.axis.x_axis, series.settings.axis.y_axis
            ))
            .legend_group_title(
                LegendGroupTitle::new().text(
                    series
                        .settings
                        .axis
                        .legend_group_title
                        .as_ref()
                        .unwrap_or(&series.settings.name),
                ),
            )
    }
}

impl<X, Y, Z> TraceExt for Box<HeatMap<X, Y, Z>>
where
    X: Clone + Serialize,
    Y: Clone + Serialize,
    Z: Clone + Serialize,
{
    fn apply_settings(self, series: &FlatSeries) -> Self {
        self.name(&series.settings.name)
            .x_axis(&series.settings.axis.x_axis)
            .y_axis(&series.settings.axis.y_axis)
            .zsmooth(Smoothing::Best)
    }
}

impl<X, Y> TraceExt for Box<Bar<X, Y>>
where
    X: Clone + Serialize,
    Y: Clone + Serialize,
{
    fn apply_settings(self, series: &FlatSeries) -> Self {
        self.name(&series.settings.name)
            .x_axis(&series.settings.axis.x_axis)
            .y_axis(&series.settings.axis.y_axis)
            .legend_group(format!(
                "x{0}y{1}",
                series.settings.axis.x_axis, series.settings.axis.y_axis
            ))
            .legend_group_title(
                LegendGroupTitle::new().text(
                    series
                        .settings
                        .axis
                        .legend_group_title
                        .as_ref()
                        .unwrap_or(&series.settings.name),
                ),
            )
    }
}

impl<X, Y> TraceExt for Box<BoxPlot<X, Y>>
where
    X: Clone + Serialize,
    Y: Clone + Serialize,
{
    fn apply_settings(self, series: &FlatSeries) -> Self {
        self.name(&series.settings.name)
            .x_axis(&series.settings.axis.x_axis)
            .y_axis(&series.settings.axis.y_axis)
            .legend_group(format!(
                "x{0}y{1}",
                series.settings.axis.x_axis, series.settings.axis.y_axis
            ))
            .legend_group_title(
                LegendGroupTitle::new().text(
                    series
                        .settings
                        .axis
                        .legend_group_title
                        .as_ref()
                        .unwrap_or(&series.settings.name),
                ),
            )
    }
}

#[derive(Debug, Error)]
pub(crate) enum ChartOutputError {
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
                match metric.get_property(series.from_bucket_block, series.property.clone()) {
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

    fn build_scalar_x_axis<T>(&self, data: &[Option<T>]) -> Vec<f64> {
        self.chart
            .x_axis
            .iter()
            .zip(data)
            .filter_map(|(a, b)| b.is_some().then_some(*a))
            .collect::<Vec<_>>()
    }

    /// Builds a trace composed from a single scalar value over the x-axis.
    ///
    /// # Arguments
    /// - series: Source of series settings
    /// - data: Slice of optional values.
    fn build_scalar_trace(&self, series: &FlatSeries, data: &[Option<f64>]) -> Box<dyn Trace> {
        let x_axis = self.build_scalar_x_axis(data);
        let y_axis = data.iter().flatten().copied().collect::<Vec<_>>();
        match &series.settings.line.series_type {
            SeriesType::Scatter(scatter_type) => Scatter::new(x_axis, y_axis)
                .mode(scatter_type.into())
                .apply_settings(series),
            SeriesType::Bar => Bar::new(x_axis, y_axis).apply_settings(series),
        }
    }

    /// Builds a trace composed from a scalar value over the x-axis as well as a symmetric error band.
    ///
    /// # Arguments
    /// - series: Source of series settings
    /// - data: Slice of optional pairs of the form `(value, error_value)`.
    fn build_scalar_with_errors_trace(
        &self,
        series: &FlatSeries,
        data: &[Option<(f64, f64)>],
    ) -> Box<dyn Trace> {
        let x_axis = self.build_scalar_x_axis(data);
        let y_axis = data.iter().flatten().map(|x| x.0).collect::<Vec<_>>();
        let band = data.iter().flatten().map(|x| x.1).collect::<Vec<_>>();
        let error_y = ErrorData::new(ErrorType::Data).array(band).thickness(0.5);
        match &series.settings.line.series_type {
            SeriesType::Scatter(scatter_type) => Scatter::new(x_axis, y_axis)
                .mode(scatter_type.into())
                .error_y(error_y)
                .apply_settings(series),
            SeriesType::Bar => Bar::new(x_axis, y_axis)
                .error_y(error_y)
                .apply_settings(series),
        }
    }

    /// Builds a trace [TODO]
    ///
    /// # Arguments
    /// - series: Source of series settings
    /// - data: FIXME: TODO
    fn build_box_plot_plotly_trace(
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
            .box_points(BoxPoints::All)
            .jitter(10.0)
            .hover_text_array(hover_text)
            .box_mean(BoxMean::True)
            .apply_settings(series)
    }

    /// Builds a trace [TODO]
    ///
    /// # Arguments
    /// - series: Source of series settings
    /// - data: FIXME: TODO
    fn build_histograms_plotly_trace(
        &self,
        series: &FlatSeries,
        data: &[HistogramWithBands],
    ) -> Vec<Box<dyn Trace>> {
        data.iter()
            .map(|histogram| {
                let mut error_y = ErrorData::new(ErrorType::Data).thickness(0.5);
                if let Some((upper, lower)) = histogram.bands.as_ref() {
                    error_y = error_y
                        .symmetric(false)
                        .array(upper.clone())
                        .array_minus(lower.clone())
                }
                let scatter = Scatter::new(histogram.labels.clone(), histogram.central.clone())
                    .error_y(error_y)
                    .apply_settings(series);

                match &series.settings.line.series_type {
                    SeriesType::Scatter(scatter_type) => {
                        scatter.mode(scatter_type.into()) as Box<dyn Trace>
                    }
                    SeriesType::Bar => scatter as Box<dyn Trace>,
                }
            })
            .collect::<Vec<_>>()
    }

    pub(crate) fn build_surface_plotly_trace(
        &self,
        series: &FlatSeries,
        surfaces: &[Surface],
    ) -> Vec<Box<dyn Trace>> {
        surfaces
            .iter()
            .map(|surface| {
                HeatMap::new(
                    surface.x_values.clone(),
                    surface.y_values.clone(),
                    surface.data.clone(),
                )
                .apply_settings(series)
                .transpose(false) as Box<dyn Trace>
            })
            .collect::<Vec<_>>()
    }

    pub(crate) fn build_plotly_trace(
        &self,
        series: &FlatSeries,
        data: Option<&MetricOutputSeries>,
    ) -> Vec<Box<dyn Trace>> {
        match data {
            Some(MetricOutputSeries::Value(data)) => vec![self.build_scalar_trace(series, data)],
            Some(MetricOutputSeries::WithErrors(data)) => {
                vec![self.build_scalar_with_errors_trace(series, data)]
            }
            Some(MetricOutputSeries::Group(data)) => {
                vec![self.build_box_plot_plotly_trace(series, data)]
            }
            Some(MetricOutputSeries::Histograms(data)) => {
                self.build_histograms_plotly_trace(series, data)
            }
            Some(MetricOutputSeries::Surface(data)) => {
                self.build_surface_plotly_trace(series, data)
            }
            None => vec![
                Scatter::<f64, f64>::new(Default::default(), Default::default())
                    .name(format!("{} - values missing.", series.settings.name)),
            ],
        }
    }

    fn build_plotly_grid(&self) -> LayoutGrid {
        let mut grid = LayoutGrid::new()
            .columns(self.chart.settings.multiplot.num_cols)
            .rows(self.chart.settings.multiplot.num_rows)
            .pattern(GridPattern::Independent);
        if let Some(x_gap_fraction) = &self.chart.settings.multiplot.x_gap_fraction {
            grid = grid.x_gap(*x_gap_fraction);
        }
        if let Some(y_gap_fraction) = &self.chart.settings.multiplot.y_gap_fraction {
            grid = grid.y_gap(*y_gap_fraction);
        }
        grid
    }

    fn build_plotly_legend(&self) -> Legend {
        Legend::new()
            .item_click(ItemClick::Toggle)
            .item_double_click(ItemClick::ToggleOthers)
            .group_click(GroupClick::ToggleItem)
            .trace_order(TraceOrder::Grouped)
            .trace_group_gap(self.chart.settings.height.unwrap_or(100))
            .valign(VAlign::Middle)
    }

    fn build_plotly_layout(&self) -> Layout {
        let mut layout = Layout::new()
            .title(&self.chart.settings.title)
            .mode_bar(ModeBar::new())
            .show_legend(true)
            .auto_size(true)
            .legend(self.build_plotly_legend())
            .grid(self.build_plotly_grid());
        if let Some(width) = &self.chart.settings.width {
            layout = layout.width((self.chart.settings.multiplot.num_cols + 1) * width);
        }
        if let Some(height) = &self.chart.settings.height {
            layout = layout.height((self.chart.settings.multiplot.num_rows + 1) * height);
        }

        // Apply x- and y-axes settings.
        const X_AXIS_METHODS: [fn(Layout, Axis) -> Layout; 8] = [
            Layout::x_axis,
            Layout::x_axis2,
            Layout::x_axis3,
            Layout::x_axis4,
            Layout::x_axis5,
            Layout::x_axis6,
            Layout::x_axis7,
            Layout::x_axis8,
        ];
        const Y_AXIS_METHODS: [fn(Layout, Axis) -> Layout; 8] = [
            Layout::y_axis,
            Layout::y_axis2,
            Layout::y_axis3,
            Layout::y_axis4,
            Layout::y_axis5,
            Layout::y_axis6,
            Layout::y_axis7,
            Layout::y_axis8,
        ];
        layout = X_AXIS_METHODS
            .iter()
            .take(self.chart.settings.multiplot.num_rows * self.chart.settings.multiplot.num_cols)
            .enumerate()
            .fold(layout, |layout, (index, x_axis)| {
                let (x_pos, _) = index.div_mod_floor(&self.chart.settings.multiplot.num_cols);
                x_axis(
                    layout,
                    Axis::new()
                        .title(&self.chart.settings.x_axis_label)
                        .anchor(format!("y{}", index + 1))
                        .domain(&[
                            x_pos as f64 / (self.chart.settings.multiplot.num_cols) as f64,
                            (x_pos + 1) as f64 / (self.chart.settings.multiplot.num_cols) as f64,
                        ]),
                )
            });
        layout = Y_AXIS_METHODS
            .iter()
            .take(self.chart.settings.multiplot.num_rows * self.chart.settings.multiplot.num_cols)
            .enumerate()
            .fold(layout, |layout, (index, y_axis)| {
                let (_, y_pos) = index.div_mod_floor(&self.chart.settings.multiplot.num_cols);
                y_axis(
                    layout,
                    Axis::new()
                        .title(&self.chart.settings.y_axis_label)
                        .anchor(format!("x{}", index + 1))
                        .domain(&[
                            y_pos as f64 / (self.chart.settings.multiplot.num_rows) as f64,
                            (y_pos + 1) as f64 / (self.chart.settings.multiplot.num_rows) as f64,
                        ]),
                )
            });
        layout
    }

    pub(crate) fn build_plotly_graph(&self) -> Plot {
        let mut plot = Plot::new();
        plot.set_layout(self.build_plotly_layout());

        // Using `fold`` rather than `for` as this fixes some compiler type-checking issues.
        let add_traces = |mut plot: Plot, (series, series_data): (_, &Option<_>)| {
            let traces = self.build_plotly_trace(series, series_data.as_ref());
            plot.add_traces(traces);
            plot
        };
        Iterator::zip(self.chart.series.iter(), self.data.iter()).fold(plot, add_traces)
    }
}
