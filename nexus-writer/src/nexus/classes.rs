//! The "NX_class" attribute appears in every hdf5 group and indicates the group's structure.

/// Encapsulates the "NX_class" attribute.
/// Any struct implementing [NexusSchematic] must define a constant [NexusClass].
/// The purpose of this is to allow the handling of nexus classes to be factored
/// out of the group structure into the [NexusGroup] object.
///
/// [NexusSchematic]: crate::nexus::NexusSchematic
/// [NexusGroup]: crate::nexus::NexusGroup
#[derive(strum::Display)]
pub(crate) enum NexusClass {
    /// The nexus class for the `DAE` group structure.
    #[strum(to_string = "NXdae")]
    Dae,
    /// The nexus class for the `Entry` group structure.
    #[strum(to_string = "NXentry")]
    Entry,
    /// The nexus class for the `EventData` group structure.
    #[strum(to_string = "NXevent_data")]
    EventData,
    /// The nexus class for the `Geometry` group structure.
    #[strum(to_string = "NXgeometry")]
    Geometry,
    /// The nexus class for the `Instrument` group structure.
    #[strum(to_string = "NXinstrument")]
    Instrument,
    /// The nexus class for the `Period` group structure.
    #[strum(to_string = "NXperiod")]
    Period,
    /// The nexus class for the `Root` group structure.
    #[strum(to_string = "NX_root")]
    Root,
    /// The nexus class for the `RunLog` group structure.
    #[strum(to_string = "NXrunlog")]
    Runlog,
    /// The nexus class for the `SELog` group structure.
    #[strum(to_string = "IXselog")]
    Selog,
    /// The nexus class for the `ValueLog` group structure.
    #[strum(to_string = "IXseblock")]
    SelogBlock,
    /// The nexus class for the `Source` group structure.
    #[strum(to_string = "NXsource")]
    Source,
    /// The nexus class for the `Sample` group structure.
    #[strum(to_string = "NXsample")]
    Sample,
    /// The nexus class for the `Log` group structure.
    #[strum(to_string = "NXlog")]
    Log,
}
