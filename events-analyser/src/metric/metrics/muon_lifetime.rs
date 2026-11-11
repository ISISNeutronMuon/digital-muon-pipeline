use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    engine::{MetricTypeHistogram, ValueError},
    metric::{CompleteMetric, FlatMetricShell, FlattenedMetric, JsonMetric, PartialMetric, PartialMetricShell, error::FittingError, utils::Histogram}
};

use digital_muon_common::Channel;
use nalgebra::DVector;
use tracing::warn;
use varpro::{
    prelude::SeparableModelBuilder, problem::SeparableProblemBuilder,
    solvers::levmar::LevMarSolver, statistics::FitStatistics,
};


use crate::{
    metric::{utils::MeanSD, outputs::PartialSeriesOutput},
    eventlists::ChannelDataByTopic,
};


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct JsonMuonLifetime {
    topic: String,
}

impl JsonMetric for JsonMuonLifetime {
    type Flattened = FlatMuonLifetime;
    type Settings = MetricTypeHistogram;

    fn flatten_metric(&self, library: &[String]) -> Result<Self::Flattened, ValueError> {
        Ok(FlatMuonLifetime {
            topic: library
                .iter()
                .enumerate()
                .find_map(|(index, this_topic)| (*this_topic == self.topic).then_some(index))
                .expect("This should never fail.")
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FlatMuonLifetime {
    topic: usize,
}

impl FlattenedMetric for FlatMuonLifetime {
    type Json = JsonMuonLifetime;
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct PartialMuonLifetime {
    histogram: HashMap<Channel, Histogram>,
}

impl PartialMetric for PartialMuonLifetime {
    type Source = FlatMuonLifetime;

    fn push(&mut self, source: &FlatMetricShell<Self::Source>, channel: Channel, collection_by_topic: &ChannelDataByTopic) {
        for (time, _) in collection_by_topic
            .get(source.flat.topic)
            .expect("Topic should exist, this should never fail.")
            .get_time_intensity()
        {
            self.histogram
                .entry(channel)
                .or_insert_with(|| Histogram::new(&source.settings))
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
#[serde(rename_all = "kebab-case")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MuonLifetimeProperty {
    TotalMean,
    TotalMeanWithSd,
    ChannelsBoxPlot,
}

impl CompleteMetric for CompletedMuonLifetime {
    type Partial = PartialMuonLifetime;
    type Error = FittingError;
    type Property = MuonLifetimeProperty;
    
    type Output = PartialSeriesOutput;

    fn aggregate(source: &PartialMetricShell<Self::Partial>) -> Result<Self, Self::Error> {
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

    fn get_property(&self, property: Self::Property) -> Result<PartialSeriesOutput, Self::Error> {
        match property {
            MuonLifetimeProperty::TotalMean => Ok(PartialSeriesOutput::Value(Some(
                self.total_lifetime
                    .as_ref()
                    .ok_or(FittingError::NoValue)?
                    .mean,
            ))),
            MuonLifetimeProperty::TotalMeanWithSd => Ok(PartialSeriesOutput::ValueWithErrors(Some((
                self.total_lifetime
                    .as_ref()
                    .ok_or(FittingError::NoValue)?
                    .mean,
                self.total_lifetime
                    .as_ref()
                    .ok_or(FittingError::NoValue)?
                    .sd,
            )))),
            MuonLifetimeProperty::ChannelsBoxPlot => Ok(PartialSeriesOutput::LabelledGroup(
                self.lifetime
                    .iter()
                    .flat_map(|(&c, v)| {
                        v.as_ref().map(|stats| (stats.mean, format!("Channel {c}")))
                    })
                    .collect(),
            )),
        }
    }
}

