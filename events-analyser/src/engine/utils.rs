use std::ops::RangeInclusive;
use serde::Deserialize;

use crate::engine::values::Number;


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

impl<T> Function<T> where T : Number {
    pub(crate) fn apply(&self, t : T) -> T {
        self.scale*t + self.value_at_zero
    }
}