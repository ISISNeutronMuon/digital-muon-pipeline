use crate::engine::FlatSeries;
use plotly::{
    Bar, BoxPlot, HeatMap, Scatter,
    common::{LegendGroupTitle, Line},
    heat_map::Smoothing,
};
use serde::Serialize;

/// Helper trait for plotly chart components.
pub(super) trait TraceExt {
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
