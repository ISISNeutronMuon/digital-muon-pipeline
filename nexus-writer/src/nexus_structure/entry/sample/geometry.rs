//! Defines [Geometry] group structure which contains details about the physical attributes of the sample being probed.
//! Currently unknown where this data is obtained from.
use crate::{
    hdf5_handlers::{GroupExt, NexusHDF5Result},
    nexus::NexusClass,
    nexus_structure::NexusSchematic,
    run_engine::ChunkSizeSettings,
};
use hdf5::{Dataset, Group};

/// Field names for [Geometry].
mod labels {
    pub(super) const NAME: &str = "name";
    pub(super) const DESCRIPTION: &str = "description";
    pub(super) const COMPONENT_INDEX: &str = "component_index";
}

/// Contains details about the physical attributes of the sample being probed.
pub(crate) struct Geometry {
    _name: Dataset,
    _description: Dataset,
    _component_index: Dataset,
}

impl NexusSchematic for Geometry {
    const CLASS: NexusClass = NexusClass::Geometry;
    type Settings = ChunkSizeSettings;

    fn build_group_structure(group: &Group, _settings: &Self::Settings) -> NexusHDF5Result<Self> {
        Ok(Self {
            _name: group.create_string_dataset(labels::NAME)?,
            _description: group.create_string_dataset(labels::DESCRIPTION)?,
            _component_index: group.create_scalar_dataset::<i32>(labels::COMPONENT_INDEX)?,
        })
    }

    fn populate_group_structure(group: &Group) -> NexusHDF5Result<Self> {
        Ok(Self {
            _name: group.get_dataset(labels::NAME)?,
            _description: group.get_dataset(labels::DESCRIPTION)?,
            _component_index: group.get_dataset(labels::COMPONENT_INDEX)?,
        })
    }
}
