use std::path::Path;
use crate::{analysis::metrics::MetricOutput, engine::FlatChart};
use plotly::{self, Layout, Plot, Scatter, common::{ErrorData, ErrorType, Line}, layout::{Axis, ModeBar}};

pub(crate) fn build_graph(file_path: &Path, chart: &FlatChart, data: &[MetricOutput<Vec<f64>>]) {
    let mut plot: Plot = Plot::new();
    let layout = Layout::new()
        .title(&chart.title)
        .mode_bar(ModeBar::new())
        .show_legend(true)
        .auto_size(true)
        .x_axis(Axis::new().title(&chart.x_axis_label))
        .y_axis(Axis::new().title(&chart.y_axis_label));
    
    plot.set_layout(layout);
    for (series, data) in Iterator::zip(chart.series.iter(), data.iter()) {
        match data {
            MetricOutput::Scalar(data) => {
                let trace = Scatter::new(chart.x_axis.clone(), data.clone())
                    .line(Line::new())
                    .name(&series.name);
                plot.add_trace(trace);
            },
            MetricOutput::ScalarWithBand(value, band) => {
                let trace = Scatter::new(chart.x_axis.clone(), value.clone())
                    .line(Line::new())
                    .name(&series.name)
                    .error_y(ErrorData::new(ErrorType::Data)
                        .array(band.clone())
                    );
                plot.add_trace(trace);
            },
        }
    }
    plot.write_html(file_path);
}