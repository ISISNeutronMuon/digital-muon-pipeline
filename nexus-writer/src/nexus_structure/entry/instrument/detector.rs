//! Defines [Geometry] group structure which contains details about the physical attributes of the sample being probed.
//! Currently unknown where this data is obtained from.
use crate::{
    hdf5_handlers::{GroupExt, NexusHDF5Result},
    nexus::NexusClass,
    nexus_structure::NexusSchematic,
    run_engine::ChunkSizeSettings,
};
use hdf5::{Dataset, Group};

/// Field names for [Detector].
mod labels {
    pub(super) const GROUPING: &str = "grouping";
}

/// Contains details about the physical attributes of the sample being probed.
pub(crate) struct Detector {
    _grouping: Dataset,
}

impl NexusSchematic for Detector {
    const CLASS: NexusClass = NexusClass::Detector;
    type Settings = ChunkSizeSettings;

    fn build_group_structure(group: &Group, settings: &Self::Settings) -> NexusHDF5Result<Self> {
        Ok(Self {
            _grouping: group
                .create_resizable_empty_dataset::<usize>(labels::GROUPING, settings.period)?,
        })
    }

    fn populate_group_structure(group: &Group) -> NexusHDF5Result<Self> {
        Ok(Self {
            _grouping: group.get_dataset(labels::GROUPING)?,
        })
    }
}
