use chrono::Utc;
use num::{
    Num, NumCast,
    traits::{NumOps, int::PrimInt},
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
pub(crate) enum JsonNumError {
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
pub(crate) enum NumExpression<T> {
    Const(T),
    FromEnvVar(String),
    NumFunc(Transformation<T>),
}
/*
impl<T : Num<FromStrRadixErr = ParseFloatError> + NumOps + NumCast + Copy> NumExpression<T> {
    pub(crate) fn value(&self, frame_index: usize) -> Result<T, JsonNumError> {
        match self {
            Self::Num(v) => Ok(*v),
            Self::NumEnv(environment_variable) => {
                Ok(Num::from_str_radix(&env::var(environment_variable)?, 10)?)
            }
            Self::NumFunc(frame_function) => {
                Ok(frame_function.transform(NumCast::from::<usize>(frame_index).ok_or(JsonNumError::UsizeConvert)?))
            }
        }
    }
}
 */
impl<T> NumExpression<T>
where
    T: Num + NumOps + NumCast + FromStr + Copy,
    JsonNumError: From<<T as FromStr>::Err>,
{
    pub(crate) fn value(&self, frame_index: usize) -> Result<T, JsonNumError> {
        match self {
            Self::Const(v) => Ok(*v),
            Self::FromEnvVar(environment_variable) => Ok(env::var(environment_variable)?.parse()?),
            Self::NumFunc(frame_function) => Ok(frame_function
                .transform(NumCast::from::<usize>(frame_index).ok_or(JsonNumError::UsizeConvert)?)),
        }
    }
}

/*impl<T : PrimInt + NumOps + NumCast + Copy> NumExpression<T> where JsonNumError : From<ParseIntError> {
    pub(crate) fn value(&self, frame_index: usize) -> Result<T, JsonNumError> {
        match self {
            Self::Num(v) => Ok(*v),
            Self::NumEnv(environment_variable) => {
                Ok(Num::from_str_radix(&env::var(environment_variable)?, 10)?)
            }
            Self::NumFunc(frame_function) => {
                Ok(frame_function.transform(NumCast::from::<usize>(frame_index).ok_or(JsonNumError::UsizeConvert)?))
            }
        }
    }
}*/
/*
#[derive(Debug, Error)]
pub(crate) enum JsonIntError {
    #[error("Cannot Extract Environment Variable")]
    EnvVar(#[from] VarError),
    #[error("Invalid String to Float: {0}")]
    FloatFromStr(#[from] ParseIntError),
}

*/
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NumConstant<T> {
    Const(T),
    FromEnvVar(String),
}

impl<T> NumConstant<T>
where
    T: Num + FromStr + Copy,
    JsonNumError: From<<T as FromStr>::Err>,
{
    pub(crate) fn value(&self) -> Result<T, JsonNumError> {
        match self {
            NumConstant::Const(v) => Ok(*v),
            NumConstant::FromEnvVar(environment_variable) => {
                Ok(env::var(environment_variable)?.parse()?)
            }
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
    pub(crate) fn value(&self) -> String {
        match self {
            TextConstant::Text(v) => v.clone(),
            TextConstant::TextEnv(environment_variable) => env::var(environment_variable).unwrap(),
        }
    }
}

/*
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NumExpression<T> where T : Debug + Deserialize + Clone {
    Int(T),
    IntEnv(String),
    IntFunc(Transformation<T>),
}

impl IntExpression {
    pub(crate) fn value(&self, frame_index: usize) -> Result<i32, JsonNumError> {
        match self {
            IntExpression::Int(v) => Ok(*v),
            IntExpression::IntEnv(environment_variable) => {
                Ok(env::var(environment_variable)?.parse()?)
            }
            IntExpression::IntFunc(frame_function) => {
                Ok(frame_function.transform(frame_index as i32))
            }
        }
    }
}
 */
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case", tag = "random-type")]
pub(crate) enum FloatRandomDistribution<T: Num> {
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

impl FloatRandomDistribution<f64> {
    pub(crate) fn sample(&self, frame_index: usize) -> Result<f64, JsonNumError> {
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
                let val = Exp::new(1.0 / lifetime.value(frame_index)?)?.sample(
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
pub(crate) enum IntRandomDistribution<T: PrimInt> {
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
    JsonNumError: From<<T as FromStr>::Err>,
{
    pub(crate) fn sample(&self, frame_index: usize) -> Result<T, JsonNumError> {
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
