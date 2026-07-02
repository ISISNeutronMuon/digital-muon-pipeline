use crate::{
    analysis::metrics::{
        CompleteMetricResultClass, FittingError, MeanSD, MetricOutput, PartialMetricResultClass, utils::Histogram
    },
    engine::{FlatAlgorithm, FlatMetricMuonLifetime, FlatWaveform, MetricProperty},
    event::ChannelData,
};
use nalgebra::DVector;
use serde::{Deserialize, Serialize};
use varpro::{prelude::SeparableModelBuilder, problem::SeparableProblemBuilder, solvers::levmar::LevMarSolver, statistics::FitStatistics};

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

fn f(x: &DVector<f64>, s: f64) -> DVector<f64> {
    x.map(|x| f64::exp(-x/s))
}

fn df_ds(x: &DVector<f64>, s: f64) -> DVector<f64> {
    x.map(|x| x/s.powi(2)*f64::exp(-x/s))
}

impl CompleteMetricResultClass for CompletedMuonLifetime {
    type Partial = MuonLifetime;
    type Error = FittingError;

    fn aggregate(source: &Self::Partial) -> Result<Self, Self::Error> {
        if source.num == 0 {
            return Ok(Self { lifetime: MeanSD { mean: 0.0, sd: 0.0 } })
        }
        //info!("bin labels: {0:?}", source.histogram.get_bin_labels());
        //info!("bin counts: {0:?}", source.histogram.get_counts());
        let initial_guess = vec![2_200.0];
        let model = SeparableModelBuilder::new(["s"])
            .independent_variable(DVector::from_vec(source.histogram.get_bin_labels().to_vec()))
            .function(["s"], f)
            .partial_deriv("s", df_ds)
            .invariant_function(|x|DVector::from_element(x.len(),1.))
            .initial_parameters(initial_guess)
            .build()?;
        let problem = SeparableProblemBuilder::new(model)
            .observations(DVector::from_vec(source.histogram.get_counts().to_vec()))
            .build()?;

        // fit the data
        let fit_result = LevMarSolver::default()
            .solve(problem)
            .map_err(FittingError::FitResult)?;
        let coefs = fit_result.nonlinear_parameters();
        //info!("{0:?}",coefs.into_iter().collect::<Vec<_>>());
        //info!("{:?}", fit_result.minimization_report);

        let lifetime = *coefs.get(0)
            .ok_or_else(||FittingError::NotEnoughCoefs(format!("{0:?}", coefs.into_iter().collect::<Vec<_>>())))?;
        let stats = FitStatistics::try_from(&fit_result)?;
        Ok(Self {
            lifetime: MeanSD {
                mean: lifetime,
                sd: stats.regression_standard_error()
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
