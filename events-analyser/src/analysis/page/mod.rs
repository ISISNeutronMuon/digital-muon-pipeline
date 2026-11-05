mod chart;
mod page;

use crate::{
    analysis::metrics::{
        CompletedMetricResult, FittingError, HistogramWithBands, MetricOutputSeries,
        MetricResultError, Surface,
    },
    engine::{FlatChart, FlatPage, FlatSeries, SeriesType},
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
use std::{fs::File, path::Path};
use thiserror::Error;

use chart::{ChartOutput, ChartOutputError};

/// Helper trait for plotly chart components.
trait TraceExt {
    /// Sets the name, x_axis, y_axis, and legend data.
    fn apply_settings(self, series: &FlatSeries) -> Self;
}

impl<X, Y> TraceExt for Box<Scatter<X, Y>>
where
    X: Clone + Serialize,
    Y: Clone + Serialize,
{
    fn apply_settings(self, series: &FlatSeries) -> Self {
        let mut line = Line::new();
        if let Some(line_style) = &series.settings.line_style {
            line = line.dash(line_style.into());
        }
        if let Some(line_colour) = &series.settings.line_colour {
            line = line.color(line_colour.to_string());
        }

        self.name(&series.settings.name)
            .line(line)
            .x_axis(&series.settings.x_axis)
            .y_axis(&series.settings.y_axis)
            .legend_group(format!(
                "x{0}y{1}",
                series.settings.x_axis, series.settings.y_axis
            ))
            .legend_group_title(
                LegendGroupTitle::new().text(
                    series
                        .settings
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
            .x_axis(&series.settings.x_axis)
            .y_axis(&series.settings.y_axis)
            .zsmooth(Smoothing::Best)
        /*.legend_group(format!(
            "x{0}y{1}",
            series.settings.x_axis, series.settings.y_axis
        ))
        .legend_group_title(
            LegendGroupTitle::new().text(
                series
                    .settings
                    .legend_group_title
                    .as_ref()
                    .unwrap_or(&series.settings.name),
            ),
        ) */
    }
}

impl<X, Y> TraceExt for Box<Bar<X, Y>>
where
    X: Clone + Serialize,
    Y: Clone + Serialize,
{
    fn apply_settings(self, series: &FlatSeries) -> Self {
        self.name(&series.settings.name)
            .x_axis(&series.settings.x_axis)
            .y_axis(&series.settings.y_axis)
            .legend_group(format!(
                "x{0}y{1}",
                series.settings.x_axis, series.settings.y_axis
            ))
            .legend_group_title(
                LegendGroupTitle::new().text(
                    series
                        .settings
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
            .x_axis(&series.settings.x_axis)
            .y_axis(&series.settings.y_axis)
            .legend_group(format!(
                "x{0}y{1}",
                series.settings.x_axis, series.settings.y_axis
            ))
            .legend_group_title(
                LegendGroupTitle::new().text(
                    series
                        .settings
                        .legend_group_title
                        .as_ref()
                        .unwrap_or(&series.settings.name),
                ),
            )
    }
}
