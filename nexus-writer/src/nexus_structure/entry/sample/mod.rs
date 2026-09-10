//! Defines [Sample] group structure which contains details about the sample which is being probed.
//! Currently unknown where this data is obtained from.
mod geometry;

use crate::{
    hdf5_handlers::{GroupExt, NexusHDF5Result},
    nexus::{DatasetUnitExt, NexusClass, NexusGroup, NexusUnits},
    nexus_structure::NexusSchematic,
    run_engine::ChunkSizeSettings,
};
use geometry::Geometry;
use hdf5::{Dataset, Group};

/// Field names for [Sample].
mod labels {
    pub(super) const NAME: &str = "name";
    pub(super) const DESCRIPTION: &str = "description";
    pub(super) const SAMPLE_TYPE: &str = "type";
    pub(super) const SITUATION: &str = "situation";
    pub(super) const GEOMETRY: &str = "geometry";
    pub(super) const THICKNESS: &str = "thickness";
    pub(super) const MASS: &str = "mass";
    pub(super) const DENSITY: &str = "density";
    pub(super) const TEMPERATURE: &str = "temperature";
    pub(super) const MAGNETIC_FIELD: &str = "magnetic_field";
}

/// Contains details about the sample being probed.
pub(crate) struct Sample {
    _name: Dataset,
    _description: Dataset,
    _sample_type: Dataset,
    _situation: Dataset,
    _geometry: NexusGroup<Geometry>,
    _thickness: Dataset,
    _mass: Dataset,
    _density: Dataset,
    _temperature: Dataset,
    _magnetic_field: Dataset,
}

impl NexusSchematic for Sample {
    const CLASS: NexusClass = NexusClass::Sample;
    type Settings = ChunkSizeSettings;

    fn build_group_structure(group: &Group, settings: &Self::Settings) -> NexusHDF5Result<Self> {
        Ok(Self {
            _name: group.create_string_dataset(labels::NAME)?,
            _description: group.create_string_dataset(labels::DESCRIPTION)?,
            _sample_type: group.create_string_dataset(labels::SAMPLE_TYPE)?,
            _situation: group.create_string_dataset(labels::SITUATION)?,
            _geometry: Geometry::build_new_group(group, labels::GEOMETRY, settings)?,
            _thickness: group
                .create_resizable_empty_dataset::<f32>(labels::THICKNESS, settings.period)?
                .with_units(NexusUnits::Millimeters)?,
            _mass: group
                .create_resizable_empty_dataset::<f32>(labels::MASS, settings.period)?
                .with_units(NexusUnits::Milligrams)?,
            _density: group
                .create_resizable_empty_dataset::<f32>(labels::DENSITY, settings.period)?
                .with_units(NexusUnits::MilligramsPerCm3)?,
            _temperature: group
                .create_scalar_dataset::<f32>(labels::TEMPERATURE)?
                .with_units(NexusUnits::Kelvin)?,
            _magnetic_field: group
                .create_scalar_dataset::<f32>(labels::MAGNETIC_FIELD)?
                .with_units(NexusUnits::Gauss)?,
        })
    }

    fn populate_group_structure(group: &Group) -> NexusHDF5Result<Self> {
        Ok(Self {
            _name: group.get_dataset(labels::NAME)?,
            _description: group.get_dataset(labels::DESCRIPTION)?,
            _sample_type: group.get_dataset(labels::SAMPLE_TYPE)?,
            _situation: group.get_dataset(labels::SITUATION)?,
            _geometry: Geometry::open_group(group, labels::GEOMETRY)?,
            _thickness: group.get_dataset(labels::THICKNESS)?,
            _mass: group.get_dataset(labels::MASS)?,
            _density: group.get_dataset(labels::DENSITY)?,
            _temperature: group.get_dataset(labels::TEMPERATURE)?,
            _magnetic_field: group.get_dataset(labels::MAGNETIC_FIELD)?,
        })
    }
}
