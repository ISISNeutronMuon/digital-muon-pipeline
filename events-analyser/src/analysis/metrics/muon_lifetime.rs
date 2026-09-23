//! Implements the Muon Lifetime Metric.
//!
//! This calculates the estimated lifetime of the muon decay process that results in the given event list times.
//! The times are placed in a histogram which is then used to fit an exponential decay function.
use crate::{
    analysis::metrics::{
        FittingError, MetricOutput,
        results::{CompleteMetricResultClass, PartialMetricResultClass},
        utils::{Histogram, MeanSD},
    },
    engine::{FlatAlgorithm, FlatMetricMuonLifetime, FlatWaveform, MuonLifetimeProperty},
    eventlists::ChannelDataByTopic,
};
use digital_muon_common::Channel;
use nalgebra::DVector;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
pub(crate) struct PartialMuonLifetime {
    source: FlatMetricMuonLifetime,
    histogram: HashMap<Channel, Histogram>,
}

impl PartialMetricResultClass for PartialMuonLifetime {
    type Source = FlatMetricMuonLifetime;
    type Complete = CompletedMuonLifetime;

    fn make_default(source: &FlatMetricMuonLifetime) -> Self {
        Self {
            source: source.clone(),
            histogram: Default::default()
        }
    }

    fn load_data(&mut self, source: &Self) {
        *self = source.clone();
    }

    fn push(
        &mut self,
        _waveform: &FlatWaveform,
        _algorithm: &FlatAlgorithm,
        channel: Channel,
        by_topic: &ChannelDataByTopic,
    ) {
        for (time, _) in by_topic
            .get(self.source.topic)
            .expect("Topic should exist, this should never fail.")
            .get_time_intensity()
        {
            self.histogram
                .entry(channel)
                .or_insert_with(|| {
                    Histogram::new(
                        self.source.histogram.num_bins,
                        &self.source.histogram.interval,
                    )
                })
                .push(*time as f64);
        }
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
    total_lifetime: Option<MeanSD>,
    lifetime: HashMap<Channel, Option<MeanSD>>,
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

impl CompletedMuonLifetime {
    fn aggregate_channel(histogram: &Histogram) -> Result<Option<MeanSD>, FittingError> {
        // Begin the fitting with the true muon lifetime.
        let initial_guess = vec![2_200.0];
        // The x-axis of the histogram.
        let independent_variables = DVector::from_vec(histogram.get_bin_labels().to_vec());

        // Set up the exponential decay model.
        let model = SeparableModelBuilder::new(["tau"])
            .independent_variable(independent_variables)
            .function(["tau"], exp_decay_function)
            .partial_deriv("tau", exp_decay_deriv_wrs_tau)
            .invariant_function(invariant_function)
            .initial_parameters(initial_guess)
            .build()?;

        // The y-axis of the histogram.
        let observations = DVector::from_vec(histogram.get_normalised_counts());
        let problem = SeparableProblemBuilder::new(model)
            .observations(observations)
            .build()?;

        // fit the data.
        let fit_result = LevMarSolver::default()
            .solve(problem)
            .map_err(|result| FittingError::FitResult(Box::new(result)))?;
        let coefs = fit_result.nonlinear_parameters();

        // Extract the lifetime parameter.
        let lifetime = *coefs
            .get(0)
            .ok_or(FittingError::LifetimeParameterUnavailable)?;

        // Validate lifetime parameter.
        if !lifetime.is_finite() {
            warn!("Infinite lifetime found");
            return Ok(None);
        }

        // Extract the standard deviation for the lifetime parameters.
        let sd = FitStatistics::try_from(&fit_result)?
            .nonlinear_parameters_variance()
            .get(0)
            .ok_or(FittingError::VarianceParameterUnavailable)?
            .sqrt();

        // Validate lifetime standard deviation.
        if !sd.is_finite() {
            warn!("Infinite variance found");
            return Ok(None);
        }

        Ok(Some(MeanSD { mean: lifetime, sd }))
    }
}

impl CompleteMetricResultClass for CompletedMuonLifetime {
    type Partial = PartialMuonLifetime;
    type Error = FittingError;
    type Property = MuonLifetimeProperty;

    fn aggregate(source: &Self::Partial) -> Result<Self, Self::Error> {
        let lifetime = source
            .histogram
            .iter()
            .map(|(channel, histogram)| {
                Ok::<_, FittingError>((*channel, Self::aggregate_channel(histogram)?))
            })
            .collect::<Result<HashMap<_, _>, _>>()?;
        let total_lifetime = lifetime.values().flatten().collect::<Option<MeanSD>>();
        Ok(Self {
            total_lifetime,
            lifetime,
        })
    }

    fn get_property(&self, property: Self::Property) -> Result<MetricOutput, Self::Error> {
        match property {
            MuonLifetimeProperty::TotalMean => Ok(MetricOutput::Value(Some(
                self.total_lifetime
                    .as_ref()
                    .ok_or(FittingError::NoValue)?
                    .mean,
            ))),
            MuonLifetimeProperty::TotalMeanWithSd => Ok(MetricOutput::WithErrors(Some((
                self.total_lifetime
                    .as_ref()
                    .ok_or(FittingError::NoValue)?
                    .mean,
                self.total_lifetime
                    .as_ref()
                    .ok_or(FittingError::NoValue)?
                    .sd,
            )))),
            MuonLifetimeProperty::ChannelsBoxPlot => Ok(MetricOutput::Group(Some(
                self.lifetime
                    .iter()
                    .flat_map(|(&c, v)| {
                        v.as_ref().map(|stats| (stats.mean, format!("Channel {c}")))
                    })
                    .collect(),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis::metrics::Histogram,
        engine::{Interval, MetricTypeHistogram},
    };

    #[test]
    fn test1() {
        let histogram_counts = [
            17488.0, 4856.0, 1410.0, 554.0, 333.0, 250.0, 225.0, 225.0, 250.0, 237.0,
        ];
        let interval = Interval {
            min: 0.0,
            max: 30000.0,
        };
        let mut histogram = Histogram::new(10, &interval);
        histogram.set(histogram_counts.to_vec());

        let source = PartialMuonLifetime {
            source: FlatMetricMuonLifetime {
                topic: 1,
                histogram: MetricTypeHistogram {
                    num_bins: 10,
                    interval,
                },
            },
            histogram: [(0, histogram)].into_iter().collect::<HashMap<_, _>>(),
        };
        let result = CompletedMuonLifetime::aggregate(&source);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(
            result.lifetime.get(&0).unwrap().as_ref().unwrap().mean,
            2269.633905415749
        );
        assert_eq!(
            result.lifetime.get(&0).unwrap().as_ref().unwrap().sd,
            8.573260580353312
        );
    }

    #[test]
    fn test2() {
        let histogram_counts = [
            7309.0, 2001.0, 542.0, 220.0, 76.0, 74.0, 43.0, 49.0, 47.0, 62.0,
        ];
        let interval = Interval {
            min: 0.0,
            max: 30000.0,
        };
        let mut histogram = Histogram::new(10, &interval);
        histogram.set(histogram_counts.to_vec());

        let source = PartialMuonLifetime {
            source: FlatMetricMuonLifetime {
                topic: 1,
                histogram: MetricTypeHistogram {
                    num_bins: 10,
                    interval,
                },
            },
            histogram: [(0, histogram)].into_iter().collect::<HashMap<_, _>>(),
        };
        let result = CompletedMuonLifetime::aggregate(&source);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(
            result.lifetime.get(&0).unwrap().as_ref().unwrap().mean,
            2273.493136014635
        );
        assert_eq!(
            result.lifetime.get(&0).unwrap().as_ref().unwrap().sd,
            16.381103849818405
        );
    }
}
