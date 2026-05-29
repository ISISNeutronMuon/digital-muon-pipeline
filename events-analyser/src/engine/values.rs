use serde::Deserialize;
use std::ops::{Add, Mul};
use thiserror::Error;

use num::NumCast;

use crate::engine::{
    Array, FlattenableWithIndex,
    utils::{Function, Interval, WithName},
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
    type Library = [WithName<Array>];
    type Error = ValueError;

    fn flatten(&self, arrays: &Self::Library, index: usize) -> Result<ConstantFilter<T>, Self::Error> {
        match self {
            ValueFilter::Dependant(dependancy) => Ok(ConstantFilter::Is(match dependancy {
                Dependancy::Array(array) => T::from(
                    arrays
                        .iter()
                        .find(|a| a.has_name(array))
                        .ok_or_else(||ValueError::CannotFindArray(array.clone()))?
                        .get_element(index),
                )
                .ok_or(ValueError::ArrayConvert)?,
                Dependancy::Function(function) => function.apply(
                    T::from(index).ok_or(ValueError::ArrayConvert)?,
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
    type Library = [WithName<Array>];
    type Error = ValueError;

    fn flatten(&self, arrays: &Self::Library, index: usize) -> Result<T, Self::Error> {
        match self {
            Value::Dependant(dependancy) => Ok(match dependancy {
                Dependancy::Array(array) => T::from(
                    arrays
                        .iter()
                        .find(|a| a.has_name(array))
                        .ok_or_else(||ValueError::CannotFindArray(array.clone()))?
                        .get_element(index),
                )
                .ok_or(ValueError::ArrayConvert)?,
                Dependancy::Function(function) => function.apply(
                    T::from(index)
                        .ok_or(ValueError::ArrayConvert)?,
                ),
            }),
            Value::Constant(constant) => Ok(*constant),
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
pub(crate) enum ValueError {
    #[error("Cannot convert array element to correct type.")]
    ArrayConvert,
    #[error("Cannot find array {0} in array list.")]
    CannotFindArray(String),
}
