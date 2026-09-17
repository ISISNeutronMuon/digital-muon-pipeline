//! Defines [Period] group structure which contains data specifying the periods used in the run.
use crate::{
    hdf5_handlers::{AttributeExt, DatasetExt, GroupExt, HasAttributesExt, NexusHDF5Result},
    nexus::NexusClass,
    nexus_structure::{NexusMessageHandler, NexusSchematic},
    run_engine::{PeriodChunkSize, run_messages::UpdatePeriodList},
};
use hdf5::{Dataset, Group};

/// Field names for [Period].
mod labels {
    pub(super) const NUMBER: &str = "number";
    pub(super) const PERIOD_TYPE: &str = "type";
    pub(super) const LABELS: &str = "labels";
    pub(super) const LABELS_SEPARATOR: &str = "separator";
    pub(super) const FRAMES_REQUESTED: &str = "frames_requested";
    pub(super) const FRAMES_REQUESTED_FRAME_TYPE: &str = "frame_type";
}

// Values of Nexus Constant
/// The character used to separate the period labels.
const LABELS_SEPARATOR: &str = ",";

/// A default constant. FIXME: This should be modifiable.
const FRAMES_REQUESTED_FRAME_TYPE: &str = "good";

/// Handles all period data.
pub(crate) struct Period {
    /// The number of periods.
    number: Dataset,

    /// Vector of period types.
    period_type: Dataset,

    /// String of [LABELS_SEPARATOR]-separated values listing all period values.
    labels: Dataset,

    /// Vector of the number of frames (per period) before switching (0 indicates unlimited).
    _frames_requested: Dataset,
}

impl Period {
    /// As periods are stored directly in the [RunParameters] object, this method extracts
    /// a vector of periods from an existing NeXus file.
    /// # Return
    /// A vector of periods.
    ///
    /// [RunParameters]: crate::run_engine::RunParameters
    pub(super) fn extract_periods(&self) -> NexusHDF5Result<Vec<u64>> {
        let separator = self
            .labels
            .get_attribute(labels::LABELS_SEPARATOR)?
            .get_string()?;
        let text = self.labels.get_string()?;
        if text.is_empty() {
            Ok(vec![])
        } else {
            text.split(&separator)
                .map(str::parse)
                .collect::<Result<_, _>>()
                .map_err(Into::into)
        }
    }
}

impl NexusSchematic for Period {
    /// The nexus class of this group.
    const CLASS: NexusClass = NexusClass::Period;

    /// This group structure only needs the appropriate chunk size.
    type Settings = PeriodChunkSize;

    fn build_group_structure(group: &Group, settings: &Self::Settings) -> NexusHDF5Result<Self> {
        Ok(Self {
            number: group.create_scalar_dataset::<u32>(labels::NUMBER)?,
            period_type: group
                .create_resizable_empty_dataset::<u32>(labels::PERIOD_TYPE, *settings)?,
            labels: group
                .create_string_dataset(labels::LABELS)?
                .with_constant_string_attribute(labels::LABELS_SEPARATOR, LABELS_SEPARATOR)?,
            _frames_requested: group
                .create_resizable_empty_dataset::<u32>(labels::FRAMES_REQUESTED, *settings)?
                .with_constant_string_attribute(
                    labels::FRAMES_REQUESTED_FRAME_TYPE,
                    FRAMES_REQUESTED_FRAME_TYPE,
                )?,
        })
    }

    fn populate_group_structure(group: &Group) -> NexusHDF5Result<Self> {
        Ok(Self {
            number: group.get_dataset(labels::NUMBER)?,
            period_type: group.get_dataset(labels::PERIOD_TYPE)?,
            labels: group.get_dataset(labels::LABELS)?,
            _frames_requested: group.get_dataset(labels::FRAMES_REQUESTED_FRAME_TYPE)?,
        })
    }
}

/// Causes the periods dataset to be rewritten from the provided period list.
impl NexusMessageHandler<UpdatePeriodList<'_>> for Period {
    fn handle_message(
        &mut self,
        UpdatePeriodList { periods }: &UpdatePeriodList<'_>,
    ) -> NexusHDF5Result<()> {
        self.number.set_scalar(&periods.len())?;
        let mut period_type = Vec::new();
        period_type.resize(periods.len(), 1);
        self.period_type.set_slice(&period_type)?;
        let separator = self
            .labels
            .get_attribute(labels::LABELS_SEPARATOR)?
            .get_string()?;
        let labels = periods
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(&separator);
        self.labels.set_string(&labels)
    }
}
