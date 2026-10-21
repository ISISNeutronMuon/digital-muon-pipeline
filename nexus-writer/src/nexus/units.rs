//! Provides helper object and trait for specifying physical units in NeXus fields.
use crate::hdf5_handlers::{DatasetExt, HasAttributesExt, NexusHDF5Result};
use hdf5::Dataset;

/// For a NeXus field which contains a physical quantity, the dataset containing the value
/// often has a "units" attribute. This object provides a single point of call
/// to manage all of these units as well as their string representations.
#[derive(strum::Display)]
pub(crate) enum NexusUnits {
    /// Measures frequency (equal to Seconds^-1).
    #[strum(to_string = "hertz")]
    Hertz,
    /// Time.
    #[strum(to_string = "second")]
    Seconds,
    /// Time (equal to 0.001 Seconds).
    #[strum(to_string = "milli.second")]
    Milliseconds,
    /// Time (equal to 0.000001 Seconds).
    #[strum(to_string = "micro.second")]
    MicroSeconds,
    /// Time (equal to 0.000001 Milliseconds).
    #[strum(to_string = "ns")]
    Nanoseconds,
    /// Energy
    #[strum(to_string = "electronvolt")]
    ElectronVolts,
    /// Energy (equal to 1,000,000 eV)
    #[strum(to_string = "mega.electronvolt")]
    MegaElectronVolts,
    /// Momentum
    #[strum(to_string = "mega.electronvolt.c^-1")]
    MegaElectronVoltsOverC,
    /// Current
    #[strum(to_string = "micro.amp")]
    MicroAmps,
    /// Charge
    #[strum(to_string = "micro.amp.hour")]
    MicroAmpHours,
    /// Length
    #[strum(to_string = "milli.metre")]
    Millimeters,
    /// Mass
    #[strum(to_string = "milli.gram")]
    Milligrams,
    /// Density
    #[strum(to_string = "milli.gram.centimetre-3")]
    MilligramsPerCm3,
    /// Temperature
    #[strum(to_string = "kelvin")]
    Kelvin,
    /// Magnetic Field
    #[strum(to_string = "gauss")]
    Gauss,
}

/// Helper trait to be implemented on [Dataset].
/// This factors out the handling of "units" attritbutes.
pub(crate) trait DatasetUnitExt: DatasetExt {
    /// Takes ownership of the dataset, add the appropriate attribute,
    /// and return the dataset to the caller.
    /// # Parameters
    ///  - units: the units to add to the dataset.
    fn with_units(self, units: NexusUnits) -> NexusHDF5Result<Dataset>;
}

impl DatasetUnitExt for Dataset {
    fn with_units(self, units: NexusUnits) -> NexusHDF5Result<Dataset> {
        self.add_constant_string_attribute("units", &units.to_string())?;
        Ok(self)
    }
}
