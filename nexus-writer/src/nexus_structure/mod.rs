//! Represents the actual Nexus file structure.
//!
//! The [entry] submodule, and all its submodules, follow the group structure
//! set out in the appropriate nexus version.
//! The [logs] submodule consists of groups that appear in the [entry] module
//! as extensible vectors of groups.

mod entry;
mod logs;

use crate::{
    hdf5_handlers::{HasAttributesExt, NexusHDF5Result},
    nexus::{NexusClass, NexusGroup, NexusMessageHandler, NexusSchematic},
    run_engine::{ChunkSizeSettings, RunParameters},
};
use chrono::{SecondsFormat, Utc};
use entry::Entry;
use hdf5::{Attribute, Group};
use std::{path::PathBuf, str::FromStr};

/// Field names for [Root].
mod labels {
    pub(super) const HDF5_VERSION: &str = "HDF5_version";
    pub(super) const NEXUS_VERSION: &str = "NeXuS_version";
    pub(super) const FILE_NAME: &str = "file_name";
    pub(super) const FILE_TIME: &str = "file_time";
    pub(super) const RAW_DATA_1: &str = "raw_data_1";
}

/// Encapsulates the top-level of a NeXus file,
pub(crate) struct Root {
    /// version of HDF library used by nexus to create file, should be set from `hdf5::HDF5_VERSION`
    _hdf5_version: Attribute,
    /// version of nexus API used in writing the file
    _nexus_version: Attribute,
    /// File name of current file, to assist identification if the external name has been changed
    _file_name: Attribute,
    /// Is set to the time this object is created
    _file_time: Attribute,
    /// All incoming data goes here
    raw_data_1: NexusGroup<Entry>,
}

impl Root {
    /// See [Entry::extract_run_parameters].
    pub(super) fn extract_run_parameters(&self) -> NexusHDF5Result<RunParameters> {
        self.raw_data_1.extract(Entry::extract_run_parameters)
    }
}

impl NexusSchematic for Root {
    const CLASS: NexusClass = NexusClass::Root;
    type Settings = ChunkSizeSettings;

    fn build_group_structure(group: &Group, settings: &ChunkSizeSettings) -> NexusHDF5Result<Self> {
        let file_path = PathBuf::from_str(&group.filename())
            .expect("Group should have filepath, this should never fail.");
        let file_name = file_path
            .file_name()
            .expect("File path should have file name, this should never fail.")
            .to_str()
            .expect("File name should be representable as str, this should never fail.");

        Ok(Self {
            _hdf5_version: group.add_constant_string_attribute(
                labels::HDF5_VERSION,
                &format!(
                    "{0}.{1}.{2}",
                    hdf5::HDF5_VERSION.major,
                    hdf5::HDF5_VERSION.minor,
                    hdf5::HDF5_VERSION.micro
                ),
            )?,
            _nexus_version: group.add_constant_string_attribute(labels::NEXUS_VERSION, "")?, // Where does this come from?
            _file_name: group.add_constant_string_attribute(labels::FILE_NAME, file_name)?,
            _file_time: group.add_constant_string_attribute(
                labels::FILE_TIME,
                Utc::now()
                    .to_rfc3339_opts(SecondsFormat::Secs, true)
                    .as_str(),
            )?,
            raw_data_1: Entry::build_new_group(group, labels::RAW_DATA_1, settings)?,
        })
    }

    fn populate_group_structure(group: &Group) -> NexusHDF5Result<Self> {
        Ok(Self {
            _hdf5_version: group.get_attribute(labels::HDF5_VERSION)?,
            _nexus_version: group.get_attribute(labels::NEXUS_VERSION)?,
            _file_name: group.get_attribute(labels::FILE_NAME)?,
            _file_time: group.get_attribute(labels::FILE_TIME)?,
            raw_data_1: Entry::open_group(group, labels::RAW_DATA_1)?,
        })
    }
}

/// Generic implementation of all traits [NexusMessageHandler\<M\>] which are implemented by [Entry].
///
/// [NexusMessageHandler\<M\>]: NexusMessageHandler
impl<M> NexusMessageHandler<M> for Root
where
    Entry: NexusMessageHandler<M>,
{
    /// Propagates messages to [Self::raw_data_1].
    fn handle_message(&mut self, message: &M) -> NexusHDF5Result<()> {
        self.raw_data_1.handle_message(message)
    }
}
