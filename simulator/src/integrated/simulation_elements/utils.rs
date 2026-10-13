use chrono::Utc;
use num::{
    Float, Num, NumCast,
    traits::{Inv, NumOps, int::PrimInt},
};
use rand::{RngExt, SeedableRng};
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

/// Represents a constant value, either a literal or from an environment variable.
///
/// Should only be created via JSON deserialisation.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NumConstant<T> {
    /// A literal constant.
    Const(T),
    /// Take constant from this environment variable.
    FromEnvVar(String),
}

impl<T> NumConstant<T>
where
    T: Num + FromStr + Copy,
    JsonValueError: From<<T as FromStr>::Err>,
{
    /// Returns the underlying value.
    pub(crate) fn value(&self) -> Result<T, JsonValueError> {
        match self {
            Self::Const(v) => Ok(*v),
            Self::FromEnvVar(environment_variable) => Ok(env::var(environment_variable)?.parse()?),
        }
    }
}

/// Represents a string value, either a literal or from an environment variable.
///
/// Should only be created via JSON deserialisation.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TextConstant {
    /// A literal constant.
    Text(String),
    /// Take constant from this environment variable.
    TextEnv(String),
}

impl TextConstant {
    /// Returns the underlying value.
    pub(crate) fn value(&self) -> Result<String, JsonValueError> {
        match self {
            Self::Text(v) => Ok(v.clone()),
            Self::TextEnv(environment_variable) => Ok(env::var(environment_variable)?),
        }
    }
}

/// Represents a numerical value, either a literal, an environment variable
/// or from a transformation applied to the current frame index.
///
/// Should only be created via JSON deserialisation.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NumExpression<T> {
    /// A literal constant expression.
    Const(T),
    /// Take constant expression from this environment variable.
    FromEnvVar(String),
    /// Take value this transformation applied to the frame number.
    NumFunc(Transformation<T>),
}

impl<T> NumExpression<T>
where
    T: Num + NumCast + FromStr + Copy,
    JsonValueError: From<<T as FromStr>::Err>,
{
    /// Returns or calculates the underlying value.
    ///
    /// # Parameters
    /// - frame_index: provided for values which depend on the frame index.
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

/// Caclulates a value from a continuous random distribution.
///
/// Should only be created via JSON deserialisation.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case", tag = "random-type")]
pub(crate) enum FloatRandomDistribution<T> {
    /// A non-random value.
    ConstantFloat { value: NumExpression<T> },
    /// A uniformly random value.
    UniformFloat {
        min: NumExpression<T>,
        max: NumExpression<T>,
    },
    /// A random normally distributed value.
    Normal {
        mean: NumExpression<T>,
        sd: NumExpression<T>,
    },
    /// A random exponentially distributed value.
    Exponential { lifetime: NumExpression<T> },
}

impl<T> FloatRandomDistribution<T>
where
    T: Float + Inv<Output = T> + FromStr + SampleUniform,
    JsonValueError: From<<T as FromStr>::Err>,
    rand_distr::StandardNormal: rand_distr::Distribution<T>,
    rand_distr::Exp1: rand_distr::Distribution<T>,
{
    /// Generate an instance of the random value.
    ///
    /// # Parameters
    /// - frame_index: provided for any fields which depend on the frame index.
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

/// Caclulates a value from a discrete random distribution.
///
/// Should only be created via JSON deserialisation.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case", tag = "random-type")]
pub(crate) enum IntRandomDistribution<T> {
    /// A non-random value.
    ConstantInt { value: NumExpression<T> },
    /// A uniformly random value.
    UniformInt {
        min: NumExpression<T>,
        max: NumExpression<T>,
    },
}

impl<T: PrimInt + FromStr + SampleUniform> IntRandomDistribution<T>
where
    JsonValueError: From<<T as FromStr>::Err>,
{
    /// Generate an instance of the random value.
    ///
    /// # Parameters
    /// - frame_index: provided for any fields which depend on the frame index.
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

/// Represents a interval of values.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Interval<T>
where
    T: Clone,
{
    /// The minimum value.
    pub(crate) min: T,
    /// The maximum value.
    pub(crate) max: T,
}

impl<T: PartialOrd + Copy> Interval<T> {
    /// Returns a [RangeInclusive] object.
    pub(crate) fn range_inclusive(&self) -> RangeInclusive<T> {
        self.min..=self.max
    }
}

impl<T> Interval<NumExpression<T>>
where
    T: PartialOrd + Num + NumCast + FromStr + Copy,
    JsonValueError: From<<T as FromStr>::Err>,
{
    /// Returns a [RangeInclusive] object.
    ///
    /// # Parameters
    /// - frame_index: provided for any fields which depend on the frame index.
    pub(crate) fn range_inclusive(
        &self,
        frame_index: usize,
    ) -> Result<RangeInclusive<T>, JsonValueError> {
        Ok(self.min.value(frame_index)?..=self.max.value(frame_index)?)
    }

    pub(crate) fn is_in(&self, value: T, frame_index: usize) -> Result<bool, JsonValueError> {
        Ok(self.range_inclusive(frame_index)?.contains(&value))
    }
}

/// Represents an affine transformation of a value, that is `x :-> scale * x + translate`.v
///
/// Should only be created via JSON deserialisation.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Transformation<T> {
    /// How much to scale the input variable.
    pub(crate) scale: T,
    /// How much to add to the scaled input variable.
    pub(crate) translate: T,
}

impl<T: NumOps + Copy> Transformation<T> {
    /// Performs the transformation on the value.
    ///
    /// # Parameters
    /// - x: value to transform.
    pub(crate) fn transform(&self, x: T) -> T {
        x * self.scale + self.translate
    }
}
