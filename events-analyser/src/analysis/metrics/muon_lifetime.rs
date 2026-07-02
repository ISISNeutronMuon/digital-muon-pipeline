//! Implements the Muon Lifetime Metric.
//!
//! This calculates the estimated lifetime of the muon decay process that results in the given event list times.
//! The times are placed in a histogram which is then used to fit an exponential decay function.
use crate::{
    analysis::metrics::{
        CompleteMetricResultClass, FittingError, MeanSD, MetricOutput, PartialMetricResultClass,
        utils::Histogram,
    },
    engine::{FlatAlgorithm, FlatMetricMuonLifetime, FlatWaveform, MetricProperty},
    eventlists::ChannelDataByTopic,
};
use nalgebra::DVector;
use serde::{Deserialize, Serialize};
use tracing::warn;
use varpro::{
    prelude::SeparableModelBuilder, problem::SeparableProblemBuilder,
    solvers::levmar::LevMarSolver, statistics::FitStatistics,
};

/// Estimates the lifetime of the muon decay process responsible for the event times.
///
/// The metric places the event times into a histogram which are used to fit
/// an exponential decay curve by [CompletedMuonLifetime].
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
            histogram: Histogram::new(source.num_bins, source.max_lifetime),
        }
    }

    fn push(
        &mut self,
        _waveform: &FlatWaveform,
        _algorithm: &FlatAlgorithm,
        by_topic: &ChannelDataByTopic,
    ) {
        self.num += 1;
        for (time, _) in by_topic
            .get(self.topic)
            .expect("This should never fail.")
            .get_time_intensity()
        {
            self.histogram.push(*time as f64);
        }
    }

    fn len(&self) -> usize {
        self.num
    }
}

/// Estimates the lifetime of the muon decay process responsible for the event times.
///
/// The aggregate function uses the histogram created by [MuonLifetime] to fit the function
/// ```latex
/// x :-> A \exp(-x/tau) + B
/// ```
/// where `A` and `B` are linear parameters and `tau` is the lifetime parameter being estimated.
/// Note we are only interested in the `tau` parameter.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CompletedMuonLifetime {
    lifetime: MeanSD,
}

/// The exponential decay function used in the fitting model.
///
/// # Parameters
/// - x: the vector of inputs.
/// - tau: the muon lifetime parameter.
fn exp_decay_function(x: &DVector<f64>, tau: f64) -> DVector<f64> {
    let neg_tau_inv = -1.0 / tau;
    x.map(|x| f64::exp(x * neg_tau_inv))
}

/// The partial derivative with respect to tau of the exponential decay function
/// used in the fitting model.
///
/// # Parameters
/// - x: the vector of inputs.
/// - tau: the muon lifetime parameter.
fn exp_decay_deriv_wrs_tau(x: &DVector<f64>, tau: f64) -> DVector<f64> {
    let neg_tau_inv = -1.0 / tau;
    let tau_sqr_inv = 1.0 / tau.powi(2);
    x.map(|x| x * tau_sqr_inv * f64::exp(x * neg_tau_inv))
}

/// A vector of ones to serve as the basis function of the linear background of the model,
/// namely `+ B` term.
///
/// # Parameters
/// - x: the vector of inputs.
fn invariant_function(x: &DVector<f64>) -> DVector<f64> {
    DVector::from_element(x.len(), 1.)
}

impl CompleteMetricResultClass for CompletedMuonLifetime {
    type Partial = MuonLifetime;
    type Error = FittingError;

    fn aggregate(source: &Self::Partial) -> Result<Self, Self::Error> {
        if source.num == 0 {
            warn!("Found null bucket");
            return Ok(Self {
                lifetime: MeanSD { mean: 0.0, sd: 0.0 },
            });
        }

        // Begin the fitting with the true muon lifetime.
        let initial_guess = vec![2_200.0];
        // The x-axis of the histogram.
        let independent_variables = DVector::from_vec(source.histogram.get_bin_labels().to_vec());

        let model = SeparableModelBuilder::new(["tau"])
            .independent_variable(independent_variables)
            .function(["tau"], exp_decay_function)
            .partial_deriv("tau", exp_decay_deriv_wrs_tau)
            .invariant_function(invariant_function)
            .initial_parameters(initial_guess)
            .build()?;

        // The y-axis of the histogram.
        let observations = DVector::from_vec(source.histogram.get_counts().to_vec());
        let problem = SeparableProblemBuilder::new(model)
            .observations(observations)
            .build()?;

        // fit the data.
        let fit_result = LevMarSolver::default()
            .solve(problem)
            .map_err(|result| FittingError::FitResult(Box::new(result)))?;
        let coefs = fit_result.nonlinear_parameters();
        println!("{:?}", fit_result.minimization_report);

        // Extract the lifetime parameter.
        let lifetime = *coefs.get(0).ok_or_else(|| {
            FittingError::NotEnoughCoefs(format!("{0:?}", coefs.into_iter().collect::<Vec<_>>()))
        })?;
        let stats = FitStatistics::try_from(&fit_result)?;
        Ok(Self {
            lifetime: MeanSD {
                mean: lifetime,
                sd: stats.regression_standard_error(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::metrics::Histogram;

    #[test]
    fn test1() {
        let histogram_counts = [
            17488.0, 4856.0, 1410.0, 554.0, 333.0, 250.0, 225.0, 225.0, 250.0, 237.0,
        ];
        let mut histogram = Histogram::new(10, 30000.0);
        histogram.set(histogram_counts.to_vec());

        let source = MuonLifetime {
            num: 1024,
            topic: 1,
            histogram,
        };
        let result = CompletedMuonLifetime::aggregate(&source);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.lifetime.mean, 2269.633905394806);
        assert_eq!(result.lifetime.sd, 22.83536314640468);
    }

    #[test]
    fn test2() {
        let histogram_counts = [
            7309.0, 2001.0, 542.0, 220.0, 76.0, 74.0, 43.0, 49.0, 47.0, 62.0,
        ];
        let mut histogram = Histogram::new(10, 30000.0);
        histogram.set(histogram_counts.to_vec());

        let source = MuonLifetime {
            num: 1024,
            topic: 1,
            histogram,
        };
        let result = CompletedMuonLifetime::aggregate(&source);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.lifetime.mean, 2273.4931383121334);
        assert_eq!(result.lifetime.sd, 18.33054952444887);
    }
}
