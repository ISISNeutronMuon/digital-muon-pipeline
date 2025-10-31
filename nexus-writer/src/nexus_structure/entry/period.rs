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
}

// Values of Nexus Constant
/// The character used to separate the period labels.
const LABELS_SEPARATOR: &str = ",";

/// Handles all period data.
pub(crate) struct Period {
    /// The number of periods.
    number: Dataset,

    /// Vector of period types.
    peroid_type: Dataset,

    /// String of [LABELS_SEPARATOR]-separated values listing all period values.
    labels: Dataset,
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
            peroid_type: group
                .create_resizable_empty_dataset::<u32>(labels::PERIOD_TYPE, *settings)?,
            labels: group
                .create_string_dataset(labels::LABELS)?
                .with_constant_string_attribute(labels::LABELS_SEPARATOR, LABELS_SEPARATOR)?,
        })
    }

    fn populate_group_structure(group: &Group) -> NexusHDF5Result<Self> {
        Ok(Self {
            number: group.get_dataset(labels::NUMBER)?,
            peroid_type: group.get_dataset(labels::PERIOD_TYPE)?,
            labels: group.get_dataset(labels::LABELS)?,
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
        let mut peroid_type = Vec::new();
        peroid_type.resize(periods.len(), 1);
        self.peroid_type.set_slice(&peroid_type)?;
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
