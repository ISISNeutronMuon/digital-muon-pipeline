use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum TraceWriterError {
    #[error("HDF5 error: {0}")]
    Hdf5(#[from] hdf5::Error),

    #[error("Missing flatbuffer field: {0}")]
    MissingField(&'static str),

    #[error("GPS timestamp conversion failed")]
    TimestampConversionFailed,

    #[error("String could not be written as HDF5 unicode: {0}")]
    UnicodeConversionFailed(String),
}
