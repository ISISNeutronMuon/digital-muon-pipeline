use crate::engine::{Array, FlattenableWithIndex, HasName};
use num::NumCast;
use serde::{Deserialize, Serialize};
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
    Constant(Filter<T>),
    /// Represents a filter that resolves to a single value filter when flattened with an index.
    Dependency(Filter<Dependency<T>>),
}

/// Represents a filter that can be applied to values.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Filter<T: Clone> {
    /// Only a single value passes this filter.
    Is(T),
    /// Only the given values passes this filter.
    AnyOf(Vec<T>),
    /// Only the values within this range pass the filter.
    AnyInRange(Interval<T>),
    /// Any value passes this filter.
    Any,
}

impl<T: Number + PartialEq + PartialOrd> Filter<T> {
    /// Tests whether a value passes this filter.
    /// # Parameters
    /// - other_value: value to test.
    pub(crate) fn is_valid(&self, other_value: T) -> bool {
        match self {
            Self::Is(value) => other_value.eq(value),
            Self::AnyOf(items) => items.iter().any(|value| other_value.eq(value)),
            Self::AnyInRange(interval) => interval.range_inclusive().contains(&other_value),
            Self::Any => true,
        }
    }
}

impl<T: Number> FlattenableWithIndex for ValueFilter<T> {
    type Flat = Filter<T>;
    type Library = [Array];
    type Error = ValueError;

    fn flatten(&self, arrays: &Self::Library, index: usize) -> Result<Filter<T>, Self::Error> {
        Ok(match self {
            ValueFilter::Dependency(dependency_filter) => match dependency_filter {
                Filter::Is(dependency) => Filter::Is(dependency.flatten(arrays, index)?),
                Filter::AnyOf(items) => Filter::AnyOf(
                    items
                        .iter()
                        .map(|dependency| dependency.flatten(arrays, index))
                        .collect::<Result<_, _>>()?,
                ),
                Filter::AnyInRange(interval) => {
                    Filter::AnyInRange(interval.flatten(arrays, index)?)
                }
                Filter::Any => Filter::Any,
            },
            ValueFilter::Constant(constant) => constant.clone(),
        })
    }
}

/// Represents a single value, either a constant hard-coded value, or one dependent on an external input.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Value<T: Number> {
    /// A constant value.
    Constant(T),
    /// Value derived from either an `Array` or `Function`.
    Dependency(Dependency<T>),
}

impl<T: Number> FlattenableWithIndex for Value<T> {
    type Flat = T;
    type Library = [Array];
    type Error = ValueError;

    fn flatten(&self, arrays: &Self::Library, index: usize) -> Result<T, Self::Error> {
        match self {
            Value::Dependency(dependency) => dependency.flatten(arrays, index),
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
            Self::Array(array) => T::from({
                let array = arrays
                    .iter()
                    .find(|a| a.has_name(array))
                    .ok_or_else(|| ValueError::CannotFindArray(array.clone()))?;
                array
                    .get_element(index)
                    .ok_or(ValueError::ArrayElement(index, array.values.len()))?
            })
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
    #[error("Element index {0} not in array of length {1}")]
    ArrayElement(usize, usize),
    #[error("Cannot find array {0} in array list.")]
    CannotFindArray(String),
}

/// Represents an end-inclusive interval of values.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Interval<T>
where
    T: Clone,
{
    min: T,
    max: T,
}

impl<T: Clone> Interval<T> {
    pub(crate) fn get_min(&self) -> T {
        self.min.clone()
    }

    pub(crate) fn get_max(&self) -> T {
        self.max.clone()
    }
}

impl<T: PartialOrd + Copy> Interval<T> {
    #[cfg(test)]
    pub(crate) fn new(min: T, max: T) -> Result<Self, ()> {
        if max < min {
            Err(())
        } else {
            Ok(Self { min, max })
        }
    }

    pub(crate) fn range_inclusive(&self) -> RangeInclusive<T> {
        self.min..=self.max
    }
}

impl<T: Number> FlattenableWithIndex for Interval<Dependency<T>> {
    type Flat = Interval<T>;
    type Library = [Array];
    type Error = ValueError;

    fn flatten(&self, library: &Self::Library, index: usize) -> Result<Self::Flat, Self::Error> {
        Ok(Interval {
            min: self.min.flatten(library, index)?,
            max: self.max.flatten(library, index)?,
        })
    }
}

impl<T: Number> FlattenableWithIndex for Interval<Value<T>> {
    type Flat = Interval<T>;
    type Library = [Array];
    type Error = ValueError;

    fn flatten(&self, library: &Self::Library, index: usize) -> Result<Self::Flat, Self::Error> {
        Ok(Interval {
            min: self.min.flatten(library, index)?,
            max: self.max.flatten(library, index)?,
        })
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
