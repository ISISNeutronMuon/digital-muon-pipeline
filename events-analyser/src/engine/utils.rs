use serde::Deserialize;
use std::{fmt::Debug, ops::RangeInclusive};

use crate::engine::values::Number;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct NameValueTemplate<T>
where
    T: Debug + Clone,
{
    name: String,
    value: T,
}

impl<T> NameValueTemplate<T>
where
    T: Debug + Clone,
{
    pub(crate) fn has_name(&self, name: &str) -> bool {
        self.name == name
    }

    pub(crate) fn get_value(&self) -> &T {
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
