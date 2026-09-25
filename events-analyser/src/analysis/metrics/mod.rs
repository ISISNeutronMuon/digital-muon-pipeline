mod event_counts;
mod false_counts;
mod muon_lifetime;
mod output;
mod pulse_height_spectra;
mod results;
mod utils;

use thiserror::Error;
use varpro::{
    fit::FitResult,
    model::{SeparableModel, SeparableNonlinearModel, builder::error::ModelBuildError},
    problem::{SeparableProblemBuilderError, SingleRhs},
    statistics::Error as StatisticsError,
};

pub(crate) use output::{HistogramWithBands, MetricOutput, MetricOutputSeries};
pub(crate) use results::{CompletedMetricResult, MetricResultError, PartialMetricResult};
#[cfg(test)]
pub(crate) use utils::Histogram;

#[derive(Debug, Error)]
pub(crate) enum FittingError {
    #[error("{0}")]
    ModelBuild(#[from] ModelBuildError),
    #[error("{0}")]
    SeparableProblemBuilder(#[from] SeparableProblemBuilderError),
    #[error("{0:?}")]
    FitResult(Box<FitResult<SeparableModel<f64>, SingleRhs>>),
    #[error("Lifetime parameter unavailable.")]
    LifetimeParameterUnavailable,
    #[error("Lifetime variance unavailable")]
    VarianceParameterUnavailable,
    #[error("Statistics Error {0}")]
    Statistics(#[from] StatisticsError<<SeparableModel<f64> as SeparableNonlinearModel>::Error>),
    #[error("No Value Present.")]
    NoValue,
}
