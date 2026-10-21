//! Defines [Instrument] group structure which contains details about the instrument used to probe the sample.
//! Currently unknown where this data is obtained from.
mod dae;
mod detector;
mod source;

use crate::{
    error::FlatBufferMissingError,
    hdf5_handlers::{DatasetExt, GroupExt, NexusHDF5Result},
    nexus::NexusClass,
    nexus_structure::{
        NexusGroup, NexusMessageHandler, NexusSchematic,
        entry::instrument::{dae::Dae, detector::Detector},
    },
    run_engine::{ChunkSizeSettings, run_messages::PushRunStart},
};
use hdf5::{Dataset, Group};
use source::Source;

/// Field names for [Instrument].
mod labels {
    pub(super) const NAME: &str = "name";
    pub(super) const SOURCE: &str = "source";
    pub(super) const DETECTOR_1: &str = "detector_1";
    pub(super) const DAE: &str = "dae";
}

/// Contains details about the instrument used to probe the sample.
pub(crate) struct Instrument {
    /// Name of the instrument.
    name: Dataset,
    /// The particle beam source used to probe the sample.
    _source: NexusGroup<Source>,
    /// The particle beam source used to probe the sample.
    _detector_1: NexusGroup<Detector>,
    /// The particle beam source used to probe the sample.
    _dae: NexusGroup<Dae>,
}

impl NexusSchematic for Instrument {
    const CLASS: NexusClass = NexusClass::Instrument;
    type Settings = ChunkSizeSettings;

    fn build_group_structure(group: &Group, settings: &Self::Settings) -> NexusHDF5Result<Self> {
        Ok(Self {
            name: group.create_string_dataset(labels::NAME)?,
            _source: Source::build_new_group(group, labels::SOURCE, &())?,
            _detector_1: Detector::build_new_group(group, labels::DETECTOR_1, settings)?,
            _dae: Dae::build_new_group(group, labels::DAE, &())?,
        })
    }

    fn populate_group_structure(group: &Group) -> NexusHDF5Result<Self> {
        Ok(Self {
            name: group.get_dataset(labels::NAME)?,
            _source: Source::open_group(group, labels::SOURCE)?,
            _detector_1: Detector::open_group(group, labels::DETECTOR_1)?,
            _dae: Dae::open_group(group, labels::DAE)?,
        })
    }
}

/// Sets the name of the instrument from a `RunStart` message.
impl NexusMessageHandler<PushRunStart<'_>> for Instrument {
    fn handle_message(
        &mut self,
        PushRunStart(run_start): &PushRunStart<'_>,
    ) -> NexusHDF5Result<()> {
        self.name.set_string(
            run_start
                .instrument_name()
                .ok_or(FlatBufferMissingError::InstrumentName)?,
        )
    }
}
