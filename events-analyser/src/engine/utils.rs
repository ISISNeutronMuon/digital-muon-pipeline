use serde::Deserialize;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut, RangeInclusive},
};

use crate::engine::values::Number;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct WithName<T>
where
    T: Debug,
{
    pub(crate) name: String,
    #[serde(flatten)]
    pub(crate) value: T,
}

impl<T> WithName<T>
where
    T: Debug,
{
    pub(crate) fn is_source<S>(&self, object: &WithSource<S>) -> bool
    where
        S: Debug,
    {
        self.name == object.source
    }

    pub(crate) fn has_name(&self, name: &str) -> bool {
        self.name == name
    }
}

impl<T> Deref for WithName<T>
where
    T: Debug,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for WithName<T>
where
    T: Debug,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct WithSource<T>
where
    T: Debug,
{
    source: String,
    #[serde(flatten)]
    value: T,
}

impl<T> WithSource<T>
where
    T: Debug,
{
    pub(crate) fn get_source(&self) -> &str {
        &self.source
    }
}

impl<T> Deref for WithSource<T>
where
    T: Debug,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
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
}

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
    pub(crate) fn apply(&self, t: T) -> T {
        self.scale * t + self.value_at_zero
    }
}
