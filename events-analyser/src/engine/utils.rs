use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut, RangeInclusive},
};

use crate::engine::values::Number;

pub(crate) trait HasSource {
    fn get_source(&self) -> &str;
}

pub(crate) trait HasName {
    fn is_source<S>(&self, object: &S) -> bool
    where
        S: HasSource,
    {
        self.get_name() == object.get_source()
    }

    fn has_name(&self, name: &str) -> bool {
        self.get_name() == name
    }

    fn get_name(&self) -> &str;
}
/*
#[derive(Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct WithName<T> {
    pub(crate) name: String,
    #[serde(flatten)]
    pub(crate) value: T,
}

impl<T: Debug> Debug for WithName<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WithName").field("name", &self.name).field("value", &self.value).finish()
    }
}

impl<T> WithName<T> {
    pub(crate) fn is_source<S>(&self, object: &WithSource<S>) -> bool
    {
        self.name == object.source
    }

    pub(crate) fn has_name(&self, name: &str) -> bool {
        self.name == name
    }
}

impl<T> Deref for WithName<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for WithName<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct WithSource<T> {
    source: String,
    #[serde(flatten)]
    value: T,
}

impl<T> WithSource<T> {
    pub(crate) fn get_source(&self) -> &str {
        &self.source
    }
}

impl<T: Debug> Debug for WithSource<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WithSource").field("source", &self.source).field("value", &self.value).finish()
    }
}

impl<T> Deref for WithSource<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
*/
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
