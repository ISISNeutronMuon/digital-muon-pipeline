use chrono::Utc;
use num::{
    Float, Num, NumCast,
    traits::{Inv, NumOps, int::PrimInt},
};
use rand::{Rng, SeedableRng};
use rand_distr::{Distribution, Exp, Normal, uniform::SampleUniform};
use serde::Deserialize;
use std::{
    env::{self, VarError},
    num::{ParseFloatError, ParseIntError},
    ops::RangeInclusive,
    str::FromStr,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum JsonValueError {
    #[error("Cannot Extract Environment Variable")]
    EnvVar(#[from] VarError),
    #[error("Invalid String to Float: {0}")]
    FloatFromStr(#[from] ParseFloatError),
    #[error("Invalid String to Int: {0}")]
    IntFromStr(#[from] ParseIntError),
    #[error("Cannot convert from usize")]
    UsizeConvert,
    #[error("Invalid Normal Distribution: {0}")]
    NormalDistribution(#[from] rand_distr::NormalError),
    #[error("Invalid Exponential Distribution: {0}")]
    ExpDistribution(#[from] rand_distr::ExpError),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NumConstant<T> {
    Const(T),
    FromEnvVar(String),
}

impl<T> NumConstant<T>
where
    T: Num + FromStr + Copy,
    JsonValueError: From<<T as FromStr>::Err>,
{
    pub(crate) fn value(&self) -> Result<T, JsonValueError> {
        match self {
            Self::Const(v) => Ok(*v),
            Self::FromEnvVar(environment_variable) => Ok(env::var(environment_variable)?.parse()?),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TextConstant {
    Text(String),
    TextEnv(String),
}

impl TextConstant {
    pub(crate) fn value(&self) -> Result<String, JsonValueError> {
        match self {
            Self::Text(v) => Ok(v.clone()),
            Self::TextEnv(environment_variable) => Ok(env::var(environment_variable)?),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NumExpression<T> {
    Const(T),
    FromEnvVar(String),
    NumFunc(Transformation<T>),
}

impl<T> NumExpression<T>
where
    T: Num + NumCast + FromStr + Copy,
    JsonValueError: From<<T as FromStr>::Err>,
{
    pub(crate) fn value(&self, frame_index: usize) -> Result<T, JsonValueError> {
        match self {
            Self::Const(v) => Ok(*v),
            Self::FromEnvVar(environment_variable) => Ok(env::var(environment_variable)?.parse()?),
            Self::NumFunc(frame_function) => Ok(frame_function.transform(
                NumCast::from::<usize>(frame_index).ok_or(JsonValueError::UsizeConvert)?,
            )),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case", tag = "random-type")]
pub(crate) enum FloatRandomDistribution<T> {
    ConstantFloat {
        value: NumExpression<T>,
    },
    UniformFloat {
        min: NumExpression<T>,
        max: NumExpression<T>,
    },
    Normal {
        mean: NumExpression<T>,
        sd: NumExpression<T>,
    },
    Exponential {
        lifetime: NumExpression<T>,
    },
}

impl<T> FloatRandomDistribution<T>
where
    T : Float + Inv<Output = T> + FromStr + SampleUniform,
    JsonValueError: From<<T as FromStr>::Err>,
    rand_distr::StandardNormal: rand_distr::Distribution<T>,
    rand_distr::Exp1: rand_distr::Distribution<T>,
{
    pub(crate) fn sample(&self, frame_index: usize) -> Result<T, JsonValueError> {
        match self {
            Self::ConstantFloat { value } => value.value(frame_index),
            Self::UniformFloat { min, max } => {
                let val =
                    rand::rngs::StdRng::seed_from_u64(Utc::now().timestamp_subsec_nanos() as u64)
                        .random_range(min.value(frame_index)?..max.value(frame_index)?);
                Ok(val)
            }
            Self::Normal { mean, sd } => {
                let val = Normal::new(mean.value(frame_index)?, sd.value(frame_index)?)?.sample(
                    &mut rand::rngs::StdRng::seed_from_u64(
                        Utc::now().timestamp_subsec_nanos() as u64
                    ),
                );
                Ok(val)
            }
            Self::Exponential { lifetime } => {
                let val = Exp::new(lifetime.value(frame_index)?.inv())?.sample(
                    &mut rand::rngs::StdRng::seed_from_u64(
                        Utc::now().timestamp_subsec_nanos() as u64
                    ),
                );
                Ok(val)
            }
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case", tag = "random-type")]
pub(crate) enum IntRandomDistribution<T> {
    ConstantInt {
        value: NumExpression<T>,
    },
    UniformInt {
        min: NumExpression<T>,
        max: NumExpression<T>,
    },
}

impl<T: PrimInt + FromStr + SampleUniform> IntRandomDistribution<T>
where
    JsonValueError: From<<T as FromStr>::Err>,
{
    pub(crate) fn sample(&self, frame_index: usize) -> Result<T, JsonValueError> {
        match self {
            Self::ConstantInt { value } => value.value(frame_index),
            Self::UniformInt { min, max } => {
                let seed = Utc::now().timestamp_subsec_nanos() as u64;
                let value = rand::rngs::StdRng::seed_from_u64(seed)
                    .random_range(min.value(frame_index)?..max.value(frame_index)?);
                Ok(value)
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Interval<T>
where
    T: Clone,
{
    pub(crate) min: T,
    pub(crate) max: T,
}

impl<T: PartialOrd + Copy> Interval<T> {
    pub(crate) fn range_inclusive(&self) -> RangeInclusive<T> {
        self.min..=self.max
    }

    pub(crate) fn is_in(&self, value: T) -> bool {
        self.range_inclusive().contains(&value)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Transformation<T> {
    pub(crate) scale: T,
    pub(crate) translate: T,
}

impl<T: NumOps + Copy> Transformation<T> {
    pub(crate) fn transform(&self, x: T) -> T {
        x * self.scale + self.translate
    }
}
