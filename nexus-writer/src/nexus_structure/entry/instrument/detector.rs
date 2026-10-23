//! Defines [Geometry] group structure which contains details about the physical attributes of the sample being probed.
//! Currently unknown where this data is obtained from.
use crate::{
    hdf5_handlers::{DatasetExt, GroupExt, HasAttributesExt, NexusHDF5Result},
    nexus::{DatasetUnitExt, NexusClass, NexusUnits},
    nexus_structure::NexusSchematic,
    run_engine::ChunkSizeSettings,
};
use hdf5::{Dataset, Group};

/// Field names for [Detector].
mod labels {
    pub(super) const GROUPING: &str = "grouping";
    pub(super) const GROUPING_PRIMARY: &str = "primary";
    pub(super) const GROUPING_NUMBER_GROUPS: &str = "number_groups";
    pub(super) const COUNTS: &str = "counts";
    pub(super) const COUNTS_LONG_NAME: &str = "long_name";
    pub(super) const COUNTS_SIGNAL: &str = "signal";
    pub(super) const COUNTS_AXES: &str = "axes";
    pub(super) const RAW_TIME: &str = "raw_time";
    pub(super) const SPECTRUM_INDEX: &str = "spectrum_index";
    pub(super) const SPECTRUM_INDEX_LONG_NAME: &str = "long_name";
}

// Values of Nexus Constant
const SPECTRUM_INDEX_LONG_NAME: &str = "spectrum number";
const COUNTS_AXES: &str = "[period_index, spectrum_index, raw_time]";
const COUNTS_LONG_NAME: &str = "positron counts";

/// Contains details about the physical attributes of the sample being probed.
pub(crate) struct Detector {
    _grouping: Dataset,
    _counts: Dataset,
    _raw_time: Dataset,
    _spectrum_index: Dataset,
}

impl NexusSchematic for Detector {
    const CLASS: NexusClass = NexusClass::Detector;
    type Settings = ChunkSizeSettings;

    fn build_group_structure(group: &Group, settings: &Self::Settings) -> NexusHDF5Result<Self> {
        let _grouping = group
            .create_resizable_empty_dataset::<usize>(labels::GROUPING, settings.period)?
            .with_constant_attribute(labels::GROUPING_PRIMARY, &1)?
            .with_constant_attribute(labels::GROUPING_NUMBER_GROUPS, &0)?;
        _grouping.append_value(0)?;

        let _counts = group
            .create_resizable_empty_dataset::<usize>(labels::COUNTS, settings.period)?
            .with_constant_attribute(labels::COUNTS_SIGNAL, &1)?
            .with_constant_string_attribute(labels::COUNTS_AXES, COUNTS_AXES)?
            .with_constant_string_attribute(labels::COUNTS_LONG_NAME, COUNTS_LONG_NAME)?;
        _counts.append_value(0)?;

        let _raw_time = group
            .create_resizable_empty_dataset::<f32>(labels::RAW_TIME, settings.period)?
            .with_units(NexusUnits::MicroSeconds)?;
        _raw_time.append_value(0.0)?;

        let _spectrum_index = group
            .create_resizable_empty_dataset::<usize>(labels::SPECTRUM_INDEX, settings.period)?
            .with_constant_string_attribute(
                labels::SPECTRUM_INDEX_LONG_NAME,
                SPECTRUM_INDEX_LONG_NAME,
            )?;
        _spectrum_index.append_value(0)?;
        Ok(Self {
            _grouping,
            _counts,
            _raw_time,
            _spectrum_index,
        })
    }

    fn populate_group_structure(group: &Group) -> NexusHDF5Result<Self> {
        Ok(Self {
            _grouping: group.get_dataset(labels::GROUPING)?,
            _spectrum_index: group.get_dataset(labels::SPECTRUM_INDEX)?,
            _raw_time: group.get_dataset(labels::RAW_TIME)?,
            _counts: group.get_dataset(labels::COUNTS)?,
        })
    }
}
