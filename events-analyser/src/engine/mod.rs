mod elements;
mod settings;
mod values;

pub(crate) use crate::engine::{
    elements::{
        Chart, FlatAlgorithm, FlatBucketBlock, FlatChart, FlatMetric, FlatMetricEventCount,
        FlatMetricFalseCount, FlatMetricType, FlatSeries, FlatWaveform, Metric, MetricProperty,
    },
    settings::{AnalysisSettings, Array, Templates},
};

/// Provides methods for flattening dependencies.
pub(crate) trait Flattenable<Lib> {
    /// Resulting Type with dependencies flattened.
    type Flat;
    /// Error type.
    type Error;

    /// Embeds any dependencies of the type.
    ///
    /// # Parameters
    /// - library: dependencies referenced by the type are passed in here.
    fn flatten(&self, library: Lib) -> Result<Self::Flat, Self::Error>;
}

/// Provides methods for flattening dependencies with additional index parameter.
trait FlattenableWithIndex {
    /// Resulting type upon flattening.
    type Flat;
    /// Structure that can be referenced during flattening.
    type Library: ?Sized;
    /// Error type.
    type Error;

    /// Embeds any dependencies of the type.
    ///
    /// # Parameters
    /// - library: dependencies referenced by the type are passed in here.
    /// - index: FIXME.
    fn flatten(&self, library: &Self::Library, index: usize) -> Result<Self::Flat, Self::Error>;
}

/// Should be defined for any structures whose fields depend on an external template.
pub(crate) trait HasSource {
    /// Returns the object's source.
    fn get_source(&self) -> &str;
}

/// Should be defined for any structures which can used as a template by a `HasSource` element.
pub(crate) trait HasName {
    /// Determines whether this object is the one referenced by a `HasSource` element.
    fn is_source<S>(&self, object: &S) -> bool
    where
        S: HasSource,
    {
        self.get_name() == object.get_source()
    }

    /// Determines whether this object has the given name.
    fn has_name(&self, name: &str) -> bool {
        self.get_name() == name
    }

    /// Returns the object's name.
    fn get_name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    //use super::*;

    //const JSON_INPUT_1: &str = r#""#;
    #[test]
    fn test1() {
        //let simulation: AnalysisSettings = serde_json::from_str(JSON_INPUT_1).unwrap();
    }
}
