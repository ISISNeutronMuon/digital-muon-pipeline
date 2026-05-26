use rand_distr::Distribution;
use serde::Deserialize;
use std::ops::{Add, Mul};
use thiserror::Error;

use num::NumCast;

use crate::engine::{
    Array, FlattenableWithIndex,
    utils::{Function, Interval},
};

pub(crate) trait Number:
    Add<Self, Output = Self> + Mul<Self, Output = Self> + Copy + Clone + NumCast
{
}

impl<T> Number for T where
    T: Add<Self, Output = Self> + Mul<Self, Output = Self> + Copy + Clone + NumCast
{
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ValueFilter<T: Number> {
    Constant(ConstantFilter<T>),
    Dependant(Dependancy<T>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ConstantFilter<T: Clone> {
    Is(T),
    AnyOf(Vec<T>),
    AnyInRange(Interval<T>),
    Any,
}

impl<T: Number + PartialEq + PartialOrd> ConstantFilter<T> {
    pub(crate) fn is_valid(&self, other_value: T) -> bool {
        match self {
            ConstantFilter::Is(value) => other_value.eq(value),
            ConstantFilter::AnyOf(items) => items.iter().any(|value| other_value.eq(value)),
            ConstantFilter::AnyInRange(interval) => {
                interval.range_inclusive().contains(&other_value)
            }
            ConstantFilter::Any => true,
        }
    }
}

impl<T: Number> FlattenableWithIndex for ValueFilter<T> {
    type Flat = ConstantFilter<T>;
    type Library = [Array];
    type Error = String;

    fn flatten(&self, arrays: &[Array], index: usize) -> Result<ConstantFilter<T>, Self::Error> {
        match self {
            ValueFilter::Dependant(dependancy) => Ok(ConstantFilter::Is(match dependancy {
                Dependancy::Array(array) => T::from(
                    arrays
                        .iter()
                        .find(|a| a.has_name(&array))
                        .ok_or_else(|| format!("Cannot find {array} in list of arrays."))?
                        .get_element(index),
                )
                .ok_or_else(|| format!("Cannot convert array element to correct type."))?,
                Dependancy::Function(function) => function.apply(
                    T::from(index)
                        .ok_or_else(|| format!("Cannot convert array element to correct type."))?,
                ),
            })),
            ValueFilter::Constant(constant) => Ok(constant.clone()),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Value<T: Number> {
    Constant(T),
    Dependant(Dependancy<T>),
}

impl<T: Number> FlattenableWithIndex for Value<T> {
    type Flat = T;
    type Library = [Array];
    type Error = String;

    fn flatten(&self, arrays: &[Array], index: usize) -> Result<T, Self::Error> {
        match self {
            Value::Dependant(dependancy) => Ok(match dependancy {
                Dependancy::Array(array) => T::from(
                    arrays
                        .iter()
                        .find(|a| a.has_name(&array))
                        .ok_or_else(|| format!("Cannot find {array} in list of arrays."))?
                        .get_element(index),
                )
                .ok_or_else(|| format!("Cannot convert array element to correct type."))?,
                Dependancy::Function(function) => function.apply(
                    T::from(index)
                        .ok_or_else(|| format!("Cannot convert array element to correct type."))?,
                ),
            }),
            Value::Constant(constant) => Ok(constant.clone()),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Dependancy<T> {
    Array(String),
    Function(Function<T>),
}

#[derive(Debug, Error)]
pub(crate) enum SimulationError {
    #[error("Event Pulse Template index {0} out of range {1}")]
    EventListIndexOutOfRange(usize, usize),
    #[error("Event Pulse Template index {0} out of range {1}")]
    EventPulseTemplateIndexOutOfRange(usize, usize),
    //#[error("Json Float error: {0}")]
    //sonValue(#[from] JsonValueError),
    //#[error("Build error: {0}")]
    //Build(#[from] BuildError),
}
