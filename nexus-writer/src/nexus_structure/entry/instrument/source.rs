//! Defines [Source] group structure which contains details about the particle source used to probe the sample.
//! Currently unknown where this data is obtained from.
use super::NexusSchematic;
use crate::{
    hdf5_handlers::{GroupExt, HasAttributesExt, NexusHDF5Result},
    nexus::{DatasetUnitExt, NexusClass, NexusUnits},
};
use hdf5::{Dataset, Group};

/// Field names for [Source].
mod labels {
    pub(super) const NAME: &str = "name";
    pub(super) const SOURCE_TYPE: &str = "type";
    pub(super) const PROBE: &str = "probe";
    pub(super) const SOURCE_FRAME_PATTERN: &str = "source_frame_pattern";
    pub(super) const SOURCE_FRAME_PATTERN_REP_LEN: &str = "rep_len";
    pub(super) const SOURCE_FRAME_PATTERN_PERIOD: &str = "period";
    pub(super) const SOURCE_FRAME_PATTERN_PULSES_PER_FRAME: &str = "pulses_per_frame";
    pub(super) const SOURCE_FREQUENCY: &str = "source_frequency";
    pub(super) const SOURCE_ENERGY: &str = "source_energy";
    pub(super) const SOURCE_CURRENT: &str = "source_current";
    pub(super) const SOURCE_PULSE_WIDTH: &str = "source_pulse_width";
    pub(super) const TARGET_MATERIAL: &str = "target_material";
    pub(super) const TARGET_THICKNESS: &str = "target_thickness";
    pub(super) const PION_MOMENTUM: &str = "pion_momentum";
    pub(super) const MUON_ENERGY: &str = "muon_energy";
    pub(super) const MUON_MOMENTUM: &str = "muon_momentum";
    pub(super) const MUON_PULSE_PATTERN: &str = "muon_pulse_pattern";
    pub(super) const MUON_PULSE_PATTERN_REP_LEN: &str = "rep_len";
    pub(super) const MUON_PULSE_PATTERN_PERIOD: &str = "period";
    pub(super) const MUON_PULSE_PATTERN_PULSES_PER_FRAME: &str = "pulses_per_frame";
    pub(super) const MUON_PULSE_WIDTH: &str = "muon_pulse_width";
    pub(super) const MUON_PULSE_SEPARATION: &str = "muon_pulse_separation";
    pub(super) const NOTES: &str = "notes";
}

// Values of Nexus Constant
/// The institution at which the source is generated.
const NAME: &str = "ISIS";
/// The type of particle beam used.
const SOURCE_TYPE: &str = "pulsed muon source";
/// The type of particle used as the probe.
const PROBE: &str = "negative muons";

/// Contains details about the particle source used to probe the sample.
pub(crate) struct Source {
    _name: Dataset,
    _source_type: Dataset,
    _probe: Dataset,
    _source_frame_pattern: Dataset,
    _source_frequency: Dataset,
    _source_energy: Dataset,
    _source_current: Dataset,
    _source_pulse_width: Dataset,
    _target_material: Dataset,
    _target_thickness: Dataset,
    _pion_momentum: Dataset,
    _muon_energy: Dataset,
    _muon_momentum: Dataset,
    _muon_pulse_pattern: Dataset,
    _muon_pulse_width: Dataset,
    _muon_pulse_separation: Dataset,
    _notes: Dataset,
}

impl NexusSchematic for Source {
    const CLASS: NexusClass = NexusClass::Source;
    type Settings = ();

    fn build_group_structure(group: &Group, _: &Self::Settings) -> NexusHDF5Result<Self> {
        let _source_frame_pattern = group
            .create_resizable_empty_dataset::<u32>(labels::SOURCE_FRAME_PATTERN, 1)?
            .with_attribute::<u32>(labels::SOURCE_FRAME_PATTERN_REP_LEN)?
            .with_attribute::<f32>(labels::SOURCE_FRAME_PATTERN_PERIOD)?
            .with_units(NexusUnits::Milliseconds)?
            .with_attribute::<f32>(labels::SOURCE_FRAME_PATTERN_PULSES_PER_FRAME)?;

        let _muon_pulse_pattern = group
            .create_resizable_empty_dataset::<u32>(labels::MUON_PULSE_PATTERN, 1)?
            .with_attribute::<u32>(labels::MUON_PULSE_PATTERN_REP_LEN)?
            .with_attribute::<f32>(labels::MUON_PULSE_PATTERN_PERIOD)?
            .with_units(NexusUnits::Milliseconds)?
            .with_attribute::<f32>(labels::MUON_PULSE_PATTERN_PULSES_PER_FRAME)?;

        Ok(Self {
            _name: group.create_constant_string_dataset(labels::NAME, NAME)?,
            _source_type: group.create_constant_string_dataset(labels::SOURCE_TYPE, SOURCE_TYPE)?,
            _probe: group.create_constant_string_dataset(labels::PROBE, PROBE)?, // TODO  Is this correct?,
            _source_frequency: group
                .create_scalar_dataset::<f32>(labels::SOURCE_FREQUENCY)?
                .with_units(NexusUnits::Hertz)?,
            _source_frame_pattern,
            _source_energy: group
                .create_scalar_dataset::<f32>(labels::SOURCE_ENERGY)?
                .with_units(NexusUnits::MegaElectronVolts)?,
            _source_current: group
                .create_scalar_dataset::<f32>(labels::SOURCE_CURRENT)?
                .with_units(NexusUnits::MicroAmps)?,
            _source_pulse_width: group
                .create_scalar_dataset::<f32>(labels::SOURCE_PULSE_WIDTH)?
                .with_string_attribute("Units")?,
            _target_material: group.create_string_dataset(labels::TARGET_MATERIAL)?,
            _target_thickness: group
                .create_scalar_dataset::<f32>(labels::TARGET_THICKNESS)?
                .with_units(NexusUnits::Millimeters)?,
            _pion_momentum: group
                .create_scalar_dataset::<f32>(labels::PION_MOMENTUM)?
                .with_units(NexusUnits::MegaElectronVoltsOverC)?,
            _muon_energy: group
                .create_scalar_dataset::<f32>(labels::MUON_ENERGY)?
                .with_units(NexusUnits::ElectronVolts)?,
            _muon_momentum: group
                .create_scalar_dataset::<f32>(labels::MUON_MOMENTUM)?
                .with_units(NexusUnits::MegaElectronVoltsOverC)?,
            _muon_pulse_pattern,
            _muon_pulse_width: group
                .create_scalar_dataset::<f32>(labels::MUON_PULSE_WIDTH)?
                .with_units(NexusUnits::Nanoseconds)?,
            _muon_pulse_separation: group
                .create_scalar_dataset::<f32>(labels::MUON_PULSE_SEPARATION)?
                .with_units(NexusUnits::Nanoseconds)?,
            _notes: group.create_string_dataset(labels::NOTES)?,
        })
    }

    fn populate_group_structure(group: &Group) -> NexusHDF5Result<Self> {
        Ok(Self {
            _name: group.get_dataset(labels::NAME)?,
            _source_type: group.get_dataset(labels::SOURCE_TYPE)?,
            _probe: group.get_dataset(labels::PROBE)?,
            _source_frame_pattern: group.get_dataset(labels::SOURCE_FRAME_PATTERN)?,
            _source_pulse_width: group.get_dataset(labels::SOURCE_PULSE_WIDTH)?,
            _source_frequency: group.get_dataset(labels::SOURCE_TYPE)?,
            _source_energy: group.get_dataset(labels::SOURCE_ENERGY)?,
            _source_current: group.get_dataset(labels::SOURCE_ENERGY)?,
            _target_material: group.get_dataset(labels::TARGET_MATERIAL)?,
            _target_thickness: group.get_dataset(labels::TARGET_THICKNESS)?,
            _pion_momentum: group.get_dataset(labels::PION_MOMENTUM)?,
            _muon_energy: group.get_dataset(labels::MUON_ENERGY)?,
            _muon_momentum: group.get_dataset(labels::MUON_MOMENTUM)?,
            _muon_pulse_pattern: group.get_dataset(labels::MUON_PULSE_PATTERN)?,
            _muon_pulse_width: group.get_dataset(labels::MUON_PULSE_WIDTH)?,
            _muon_pulse_separation: group.get_dataset(labels::MUON_PULSE_SEPARATION)?,
            _notes: group.get_dataset(labels::MUON_MOMENTUM)?,
        })
    }
}
