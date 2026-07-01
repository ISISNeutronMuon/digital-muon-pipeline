use crate::{
    analysis::metrics::{
        CompleteMetricResultClass, MeanSD, MetricOutput, PartialMetricResultClass, utils::Histogram,
    },
    engine::{FlatAlgorithm, FlatMetricMuonLifetime, FlatWaveform, MetricProperty},
    event::ChannelData,
};
use nalgebra::DVector;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;
use varpro::{fit::FitResult, model::{SeparableModel, SeparableNonlinearModel, builder::error::ModelBuildError}, prelude::SeparableModelBuilder, problem::{SeparableProblemBuilder, SeparableProblemBuilderError, SingleRhs}, solvers::levmar::LevMarSolver, statistics::FitStatistics};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MuonLifetime {
    num: usize,
    topic: usize,
    histogram: Histogram,
}

impl PartialMetricResultClass for MuonLifetime {
    type Source = FlatMetricMuonLifetime;
    type Complete = CompletedMuonLifetime;

    fn make_default(source: &FlatMetricMuonLifetime) -> Self {
        Self {
            num: Default::default(),
            topic: source.topic,
            histogram: Histogram::new(source.num_bins, source.max_lifetime)
        }
    }

    fn push(
        &mut self,
        _waveform: &FlatWaveform,
        _algorithm: &FlatAlgorithm,
        by_topic: &[ChannelData],
    ) {
        self.num += 1;
        for (time, _) in by_topic.get(self.topic).expect("This should never fail.").get_time_intensity() {
            self.histogram.push(*time as f64);
        }
    }

    fn len(&self) -> usize {
        self.num
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CompletedMuonLifetime {
    lifetime: MeanSD,
}

/*
*/

fn f(x: &DVector<f64>, s: f64, a: f64, b: f64) -> DVector<f64> {
    x.map(|x| a*f64::exp(-s*x) + b)
}

fn df_da(x: &DVector<f64>, s: f64, _: f64, _: f64) -> DVector<f64> {
    x.map(|x| f64::exp(-s*x))
}

fn df_db(x: &DVector<f64>, _: f64, _: f64, _: f64) -> DVector<f64> {
    x.map(|_| 1.0)
}

fn df_ds(x: &DVector<f64>, s: f64, a: f64, _: f64) -> DVector<f64> {
    x.map(|x| -x*a*f64::exp(-s*x))
}

/*
fn f(x: &DVector<f64>, a: f64, s: f64) -> DVector<f64> {
    x.map(|x| a*f64::exp(-s*x))
}

fn df_da(x: &DVector<f64>, _: f64, s: f64) -> DVector<f64> {
    x.map(|x| f64::exp(-s*x))
}

fn df_ds(x: &DVector<f64>, a: f64, s: f64) -> DVector<f64> {
    x.map(|x| -x*a*f64::exp(-s*x))
}
 */

#[derive(Debug, Error)]
pub(crate) enum MuonLifetimeError {
    #[error("{0}")]
    ModelBuild(#[from] ModelBuildError),
    #[error("{0}")]
    SeparableProblemBuilder(#[from] SeparableProblemBuilderError),
    #[error("{0:?}")]
    FitResult(FitResult<SeparableModel<f64>, SingleRhs>),
    #[error("Not enough linear coefficients: {0}")]
    NotEnoughCoefs(String),
    #[error("Statistics Error {0}")]
    Statistics(#[from] varpro::statistics::Error<<SeparableModel<f64> as SeparableNonlinearModel>::Error>)
}

impl CompleteMetricResultClass for CompletedMuonLifetime {
    type Partial = MuonLifetime;
    type Error = MuonLifetimeError;

    fn aggregate(source: &Self::Partial) -> Result<Self, Self::Error> {
        if source.num == 0 {
            return Ok(Self { lifetime: MeanSD { mean: 0.0, sd: 0.0 } })
        }
        //info!("bin labels: {0:?}", source.histogram.get_bin_labels());
        //info!("bin counts: {0:?}", source.histogram.get_counts());
        let counts = source.histogram.get_counts();
        let max_count = *counts.iter()
            .max_by(|x,y|f64::partial_cmp(x, y)
                .expect("")
            )
            .expect("");
        let model = SeparableModelBuilder::new(["s", "a", "b"])
            .independent_variable(DVector::from_vec(source.histogram.get_bin_labels().to_vec()))
            .function(["s", "a", "b"], f)
            .partial_deriv("s", df_ds)
            .partial_deriv("a", df_da)
            .partial_deriv("b", df_db)
            .initial_parameters(vec![1.0/2200.0, max_count, 0.0])
            .build()?;
        let problem = SeparableProblemBuilder::new(model)
            .observations(DVector::from_vec(counts.to_vec()))
            .build()?;

        // fit the data
        let fit_result = LevMarSolver::default()
            .solve(problem)
            .map_err(MuonLifetimeError::FitResult)?;
        let coefs = fit_result.nonlinear_parameters();
        //info!("{0:?}",coefs.into_iter().collect::<Vec<_>>());
        //info!("{:?}", fit_result.minimization_report);

        let lifetime = *coefs.get(0)
            .ok_or_else(||MuonLifetimeError::NotEnoughCoefs(format!("{0:?}", coefs.into_iter().collect::<Vec<_>>())))?;
            //.expect("This should never fail.");
        //let stats = FitStatistics::try_from(&fit_result)?;
            //.expect("This should never fail.");
        Ok(Self {
            lifetime: MeanSD {
                mean: 1.0/lifetime,
                sd: 0.0 //stats.regression_standard_error()
            },
        })
    }

    fn get_property(&self, property: &MetricProperty) -> Result<MetricOutput<f64>, String> {
        match property {
            MetricProperty::Mean => Ok(MetricOutput::Scalar(self.lifetime.mean)),
            MetricProperty::SD => Ok(MetricOutput::ScalarWithBand(
                self.lifetime.mean,
                self.lifetime.sd,
            )),
            _ => unreachable!(),
        }
    }
}
