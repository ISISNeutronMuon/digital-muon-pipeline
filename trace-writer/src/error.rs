use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum TraceWriterError {
    #[error("HDF5 error: {0}")]
    Hdf5(#[from] hdf5::Error),

    #[error("Missing flatbuffer field: {0}")]
    MissingField(&'static str),

    #[error("GPS timestamp conversion failed")]
    TimestampConversionFailed,

    #[error("Timestamp count not be converted to nanoseconds since epoch: {0}")]
    NanosecondConversionFailed(DateTime<Utc>)
}
