use crate::engine::{Array, FlattenableWithIndex, HasName};
use num::NumCast;
use serde::Deserialize;
use std::ops::{Add, Mul, RangeInclusive};
use thiserror::Error;

/// Represents any type that can be used in calculations and cast into scalars.
pub(crate) trait Number:
    Add<Self, Output = Self> + Mul<Self, Output = Self> + Copy + Clone + NumCast
{
}

impl<T> Number for T where
    T: Add<Self, Output = Self> + Mul<Self, Output = Self> + Copy + Clone + NumCast
{
}

/// Represents a filter than can either be a `ConstantFilter`, or a `Dependency` filter.
/// A `Dependency` filter is one that resolves to a `ConstantFilter::Is` filter when
/// flattened with an index.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ValueFilter<T: Number> {
    /// Represents a filter with no external dependencies.
    Constant(ConstantFilter<T>),
    /// Represents a filter that resolves to a single value filter when flattened with an index.
    Dependent(Dependency<T>),
}

/// Represents a filter that can be applied to values.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ConstantFilter<T: Clone> {
    /// Only a single value passes this filter.
    Is(T),
    /// Only the given values passes this filter.
    AnyOf(Vec<T>),
    /// Only the values within this range pass the filter.
    AnyInRange(Interval<T>),
    /// Any value passes this filter.
    Any,
}

impl<T: Number + PartialEq + PartialOrd> ConstantFilter<T> {
    /// Tests whether a value passes this filter.
    /// # Parameters
    /// - other_value: value to test.
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
    type Error = ValueError;

    fn flatten(
        &self,
        arrays: &Self::Library,
        index: usize,
    ) -> Result<ConstantFilter<T>, Self::Error> {
        match self {
            ValueFilter::Dependent(dependency) => {
                Ok(ConstantFilter::Is(dependency.flatten(arrays, index)?))
            }
            ValueFilter::Constant(constant) => Ok(constant.clone()),
        }
    }
}

/// Represents a single value, either a constant hard-coded value, or one dependent on an external input.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Value<T: Number> {
    /// A constant value.
    Constant(T),
    /// Value derived from either an `Array` or `Function`.
    Dependent(Dependency<T>),
}

impl<T: Number> FlattenableWithIndex for Value<T> {
    type Flat = T;
    type Library = [Array];
    type Error = ValueError;

    fn flatten(&self, arrays: &Self::Library, index: usize) -> Result<T, Self::Error> {
        match self {
            Value::Dependent(dependency) => dependency.flatten(arrays, index),
            Value::Constant(constant) => Ok(*constant),
        }
    }
}

/// Represents a value dependent on an external index.
/// The particular value is determined either from a hard-coded array, or a function of the index.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Dependency<T> {
    /// Takes values from an array defined in `Templates`.
    Array(String),
    /// Takes values defined by a linear function of the index.
    Function(Function<T>),
}

impl<T: Number> FlattenableWithIndex for Dependency<T> {
    type Flat = T;
    type Library = [Array];
    type Error = ValueError;

    fn flatten(&self, arrays: &Self::Library, index: usize) -> Result<Self::Flat, Self::Error> {
        Ok(match self {
            Self::Array(array) => T::from(
                arrays
                    .iter()
                    .find(|a| a.has_name(array))
                    .ok_or_else(|| ValueError::CannotFindArray(array.clone()))?
                    .get_element(index),
            )
            .ok_or(ValueError::ArrayConvert)?,
            Self::Function(function) => {
                function.apply(T::from(index).ok_or(ValueError::ArrayConvert)?)
            }
        })
    }
}

#[derive(Debug, Error)]
pub(crate) enum ValueError {
    #[error("Cannot convert array element to correct type.")]
    ArrayConvert,
    #[error("Cannot find array {0} in array list.")]
    CannotFindArray(String),
}

/// Represents an end-inclusive interval of values.
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
}

/// Represents a linear function that typically operates on an index.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Function<T> {
    scale: T,
    value_at_zero: T,
}

impl<T> Function<T>
where
    T: Number,
{
    /// Calculates the result of `|input| self.scale*input + self.value_at_zero`.
    pub(crate) fn apply(&self, t: T) -> T {
        self.scale * t + self.value_at_zero
    }
}
