use crate::{
    analysis::metrics::{
        CompleteMetricResultClass, MeanSD, MetricOutput, PartialMetricResultClass, utils::Histogram,
    },
    engine::{FlatAlgorithm, FlatMetricIntensityGraph, FlatWaveform, MetricProperty},
    event::ChannelData,
};
use nalgebra::DVector;
use serde::{Deserialize, Serialize};
use varpro::{prelude::SeparableModelBuilder, problem::SeparableProblemBuilder, solvers::levmar::LevMarSolver, statistics::FitStatistics};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct IntensityGraph {
    num: usize,
    topic: usize,
    histogram: Histogram,
}

impl PartialMetricResultClass for IntensityGraph {
    type Source = FlatMetricIntensityGraph;
    type Complete = CompletedIntensityGraph;

    fn make_default(source: &FlatMetricIntensityGraph) -> Self {
        Self {
            num: Default::default(),
            topic: source.topic,
            histogram: Histogram::new(source.num_bins, source.max_amplitude)
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
pub(crate) struct CompletedIntensityGraph {
    lifetime: MeanSD,
}

fn gaussian(x: f64, v: f64, c: f64) -> f64 {
    f64::exp(-v*(x - c).powi(2))
}

fn f(x: &DVector<f64>, a: f64, b: f64, s: f64, t: f64, y: f64, z: f64) -> DVector<f64> {
    x.map(|x| a*gaussian(x, s, y) + b*gaussian(x,t,z))
}

fn df_da(x: &DVector<f64>, _: f64, _: f64, s: f64, _: f64, y: f64, _: f64) -> DVector<f64> {
    x.map(|x| gaussian(x, s, y))
}

fn df_db(x: &DVector<f64>, _: f64, _: f64, _: f64, t: f64, _: f64, z: f64) -> DVector<f64> {
    x.map(|x| gaussian(x, t, z))
}

fn df_ds(x: &DVector<f64>, a: f64, _: f64, s: f64, _: f64, y: f64, _: f64) -> DVector<f64> {
    x.map(|x| -a*(x - y).powi(2)*gaussian(x, s, y))
}

fn df_dt(x: &DVector<f64>, _: f64, b: f64, _: f64, t: f64, _: f64, z: f64) -> DVector<f64> {
    x.map(|x| -b*(x - z).powi(2)*gaussian(x, t, z))
}

fn df_dy(x: &DVector<f64>, a: f64, _: f64, s: f64, _: f64, y: f64, _: f64) -> DVector<f64> {
    x.map(|x| 2.0*s*(x - y)*a*gaussian(x, s, y))
}

fn df_dz(x: &DVector<f64>, a: f64, _: f64, _: f64, t: f64, _: f64, z: f64) -> DVector<f64> {
    x.map(|x| 2.0*t*(x - z)*a*gaussian(x, t, z))
}


impl CompleteMetricResultClass for CompletedIntensityGraph {
    type Partial = IntensityGraph;
    type Error = ();

    fn aggregate(source: &Self::Partial) -> Result<Self,()> {
        let model = SeparableModelBuilder::new(["a", "b", "s", "t", "y", "z"])
            .independent_variable(DVector::from_vec(source.histogram.get_bin_labels().to_vec()))
            .function(["a", "b", "s", "t", "y", "z"], f)
            .partial_deriv("a", df_da)
            .partial_deriv("b", df_db)
            .partial_deriv("s", df_ds)
            .partial_deriv("t", df_dt)
            .partial_deriv("y", df_dy)
            .partial_deriv("z", df_dz)
            .initial_parameters(vec![1.0, 1.0, 1.0])
            .build()
            .expect("This should never fail.");
        let problem = SeparableProblemBuilder::new(model)
            .observations(DVector::from_vec(source.histogram.get_counts().to_vec()))
            .build()
            .expect("This should never fail.");

        // fit the data
        let fit_result = LevMarSolver::default()
            .solve(problem)
            .expect("Fit must succeed, this should never fail");
        let lifetime = *fit_result.linear_coefficients()
            .expect("This should never fail.")
            .get(2)
            .expect("This should never fail.");
        let stats = FitStatistics::try_from(&fit_result)
            .expect("This should never fail.");
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
