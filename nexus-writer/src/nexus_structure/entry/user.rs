//! Defines [Period] group structure which contains data specifying the periods used in the run.
use crate::{
    hdf5_handlers::{GroupExt, NexusHDF5Result},
    nexus::NexusClass,
    nexus_structure::NexusSchematic,
};
use hdf5::{Dataset, Group};

/// Field names for [Period].
mod labels {
    pub(super) const NAME: &str = "name";
}

/// Handles all period data.
pub(crate) struct User {
    /// The name of the user.
    _name: Dataset,
}

impl NexusSchematic for User {
    /// The nexus class of this group.
    const CLASS: NexusClass = NexusClass::User;

    /// This group structure only needs the appropriate chunk size.
    type Settings = ();

    fn build_group_structure(group: &Group, _: &Self::Settings) -> NexusHDF5Result<Self> {
        Ok(Self {
            _name: group.create_constant_string_dataset(labels::NAME, "Dr Default")?, // FIXME
        })
    }

    fn populate_group_structure(group: &Group) -> NexusHDF5Result<Self> {
        Ok(Self {
            _name: group.get_dataset(labels::NAME)?,
        })
    }
}