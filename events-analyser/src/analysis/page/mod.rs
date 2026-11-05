mod chart;
mod page;

use crate::engine::FlatSeries;

pub(crate) use page::{PageOutput, PageOutputError};

/// Helper trait for plotly chart components.
trait TraceExt {
    /// Sets the name, x_axis, y_axis, and legend data.
    fn apply_settings(self, series: &FlatSeries) -> Self;
}
