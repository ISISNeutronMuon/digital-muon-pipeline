//! Defines [DAE] group structure which contains details about the data acquistion electronics.
//! Currently unknown where this data is obtained from.
use super::NexusSchematic;
use crate::{
    hdf5_handlers::{GroupExt, NexusHDF5Result},
    nexus::NexusClass,
};
use hdf5::{Dataset, Group};

/// Field names for [DAE].
mod labels {
    pub(super) const DAE_TYPE: &str = "type";
}

// Values of Nexus Constant
/// The type of DAE used.
const DAE_TYPE: &str = "ISIS_DAE2";

/// Contains details about the data acquistion electronics used.
pub(crate) struct Dae {
    _dae_type: Dataset,
}

impl NexusSchematic for Dae {
    const CLASS: NexusClass = NexusClass::Dae;
    type Settings = ();

    fn build_group_structure(group: &Group, _: &Self::Settings) -> NexusHDF5Result<Self> {
        let _dae_type = group.create_constant_string_dataset(labels::DAE_TYPE, DAE_TYPE)?;

        Ok(Self { _dae_type })
    }

    fn populate_group_structure(group: &Group) -> NexusHDF5Result<Self> {
        Ok(Self {
            _dae_type: group.get_dataset(labels::DAE_TYPE)?,
        })
    }
}
