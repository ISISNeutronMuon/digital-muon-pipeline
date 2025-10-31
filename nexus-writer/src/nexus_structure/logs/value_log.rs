//! Implements the [ValueLog] struct which represents a NeXus group of class `IXseblock`.

use super::{AlarmLog, Log, LogSettings};
use crate::{
    hdf5_handlers::NexusHDF5Result,
    nexus::{LogMessage, NexusClass, NexusGroup, NexusMessageHandler, NexusSchematic},
    run_engine::run_messages::{PushAlarm, PushSampleEnvironmentLog},
};
use hdf5::Group;

/// Field names for [ValueLog].
mod labels {
    pub(super) const VALUE_LOG: &str = "value_log";
}

pub(crate) struct ValueLog {
    group: Group,
    alarm: Option<AlarmLog>,
    log: Option<NexusGroup<Log>>,
}

impl NexusSchematic for ValueLog {
    /// The nexus class of this group.
    const CLASS: NexusClass = NexusClass::SelogBlock;

    /// This group structure doesn't require any settings when built.
    type Settings = ();

    fn build_group_structure(group: &Group, _: &Self::Settings) -> NexusHDF5Result<Self> {
        Ok(Self {
            group: group.clone(),
            alarm: None,
            log: None,
        })
    }

    fn populate_group_structure(group: &Group) -> NexusHDF5Result<Self> {
        Ok(Self {
            group: group.clone(),
            alarm: AlarmLog::populate_group_structure(group).ok(),
            log: Log::open_group(group, labels::VALUE_LOG).ok(),
        })
    }
}

impl NexusMessageHandler<PushSampleEnvironmentLog<'_>> for ValueLog {
    /// Appends timestamps and values to the appropriate datasets.
    /// # Error Modes
    /// - Propagates errors from [Log::build_group_structure()].
    /// - Propagates errors from [Log::handle_message()].
    fn handle_message(&mut self, message: &PushSampleEnvironmentLog<'_>) -> NexusHDF5Result<()> {
        if self.log.is_none() {
            self.log = Some(Log::build_new_group(
                &self.group,
                labels::VALUE_LOG,
                &LogSettings {
                    type_descriptor: message.get_type_descriptor()?,
                    chunk_size: message.settings.selog,
                },
            )?);
        }

        self.log
            .as_mut()
            .expect("log exists, this shouldn't fail")
            .handle_message(message)
    }
}

impl NexusMessageHandler<PushAlarm<'_>> for ValueLog {
    /// If the alarm structure exists, appends the alarm data to it,
    /// otherwise create it and append.
    /// # Error Modes
    /// - Propagates errors from [AlarmLog::build_group_structure()].
    /// - Propagates errors from [AlarmLog::handle_message()].
    #[tracing::instrument(skip_all, level = "debug", err(level = "warn"))]
    fn handle_message(&mut self, message: &PushAlarm<'_>) -> NexusHDF5Result<()> {
        if self.alarm.is_none() {
            self.alarm = Some(AlarmLog::build_group_structure(
                &self.group,
                &message.settings.alarm,
            )?);
        }

        self.alarm
            .as_mut()
            .expect("alarm exists, this shouldn't happen")
            .handle_message(message)
    }
}
