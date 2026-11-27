//! HDF5 file writer for digitiser analog trace data.
//!
//! Each [TraceFileWriter] owns one open HDF5 file. Digitiser groups and channel datasets
//! are created lazily on first use.
//!
//! # HDF5 layout
//! ```text
//! /
//!   digitiser_{id}/
//!     frame_number  : [u32]              – one entry per received message
//!     timestamp     : [VarLenUnicode]    – RFC3339 timestamp per message
//!     period_number : [u64]              – one entry per received message
//!     channel_{n}   : [VarLenArray<u16>] – voltage trace per message (variable length)
//! ```

use crate::{digitiser_data::DigitizerData, error::TraceWriterError};
use chrono::{DateTime, Utc};
use digital_muon_common::FrameNumber;
use digital_muon_streaming_types::{
    dat2_digitizer_analog_trace_v2_generated::DigitizerAnalogTraceMessage,
    frame_metadata_v2_generated::FrameMetadataV2,
};
use hdf5::{Extents, File};
use std::{collections::HashMap, path::Path};

struct FirstAndLatest<T> {
    first: T,
    latest: T,
}

impl FirstAndLatest<DateTime<Utc>> {
    pub(crate) fn get_difference_ms(&self) -> i64 {
        (self.latest - self.first).num_milliseconds()
    }
}

impl FirstAndLatest<FrameNumber> {
    pub(crate) fn get_frames_difference(&self) -> i32 {
        (self.latest - self.first) as i32
    }
}

/// Manages one open HDF5 file for writing digitiser trace data.
///
/// Digitiser groups and channel datasets are created lazily the first time
/// a message from that digitiser / channel is received.
pub(crate) struct TraceFileWriter {
    /// The underlying HDF5 file.
    file: File,
    /// The chunk size to use for datasets.
    chunk_size: usize,
    /// Map of digitisers that have previously been encountered.
    digitizers: HashMap<u8, DigitizerData>,
    /// Timestamp of first and latest message received.
    timestamp: Option<FirstAndLatest<DateTime<Utc>>>,
    /// Frame number of first and latest message received.
    frame_number: Option<FirstAndLatest<FrameNumber>>,
}

impl TraceFileWriter {
    /// Creates a new HDF5 file at `path` and prepares it for writing.
    ///
    /// # Parameters
    /// - path: the path to write to.
    /// - chunk_size: chunk size for new datasets.
    pub(crate) fn new(path: &Path, chunk_size: usize) -> Result<Self, TraceWriterError> {
        let file = File::create(path)?;
        file.new_attr::<bool>()
            .shape(Extents::Scalar)
            .create("config_timestamp_as_rfc3339")?
            .write_scalar(&false)?;
        file.new_attr::<bool>()
            .shape(Extents::Scalar)
            .create("config_multiple_channel_datasets")?
            .write_scalar(&false)?;
        Ok(Self {
            file,
            chunk_size,
            digitizers: HashMap::new(),
            timestamp: None,
            frame_number: None,
        })
    }

    #[cfg(test)]
    /// Creates a new HDF5 file purely in memory.
    ///
    /// Only used for testing.
    pub(crate) fn new_temp(name: &str) -> Result<Self, TraceWriterError> {
        let mut builder = File::with_options();
        builder.fapl().core();
        let file = builder.create(name)?;
        Ok(Self {
            file,
            chunk_size: 1,
            digitizers: HashMap::new(),
            timestamp: None,
            frame_number: None,
        })
    }

    /// Writes one [DigitizerAnalogTraceMessage] into the file.
    ///
    /// The digitiser group and any required channel datasets are created on first use.
    pub(crate) fn write_trace_message(
        &mut self,
        msg: &DigitizerAnalogTraceMessage<'_>,
    ) -> Result<(), TraceWriterError> {
        let digitizer_id = msg.digitizer_id();

        if !self.digitizers.contains_key(&digitizer_id) {
            // `&self.file` coerces to `&Group` via Deref; the returned DigitizerData
            // owns its HDF5 handles independently of `self.file`.
            let dig_data = DigitizerData::new(&self.file, digitizer_id, self.chunk_size)?;
            self.digitizers.insert(digitizer_id, dig_data);
        }

        let dig_data = self
            .digitizers
            .get_mut(&digitizer_id)
            .expect("digitizer was just inserted");
        dig_data.write_trace(msg, self.chunk_size)?;

        self.set_termination_data(&msg.metadata());
        Ok(())
    }

    /// Extracts and sets the termination criteria data from the metadata of a message.
    /// That is, sets the latest timestamp and frame number (and initials if necessary)
    /// from the given metadata.
    ///
    /// # Parameters
    /// - metadata: metadata of the latest message received.
    ///
    /// # Assumptions
    /// As this method should be called after the initial trace write,
    /// this method assumes the timestamp is valid and present.
    fn set_termination_data(&mut self, metadata: &FrameMetadataV2) {
        // Optain timestamp if possible.
        let new_latest_timestamp: DateTime<_> = metadata
            .timestamp()
            .cloned()
            .expect("Timestamp should exist, this should never fail.")
            .try_into()
            .expect("Timestamp should be valid, this should never fail.");

        if let Some(first_and_latest) = &mut self.timestamp {
            // Set latest timestamp only.
            first_and_latest.latest = new_latest_timestamp;
        } else {
            // Set initial and latest timestamps.
            self.timestamp = Some(FirstAndLatest {
                first: new_latest_timestamp,
                latest: new_latest_timestamp,
            });
        }

        if let Some(first_and_latest) = &mut self.frame_number {
            // Set latest frame number only.
            first_and_latest.latest = metadata.frame_number();
        } else {
            // Set initial and latest frame numbers.
            self.frame_number = Some(FirstAndLatest {
                first: metadata.frame_number(),
                latest: metadata.frame_number(),
            });
        }
    }

    pub(crate) fn get_duration_ms(&self) -> Option<i64> {
        self.timestamp
            .as_ref()
            .map(FirstAndLatest::get_difference_ms)
    }

    pub(crate) fn get_frame_count(&self) -> Option<i32> {
        self.frame_number
            .as_ref()
            .map(FirstAndLatest::get_frames_difference)
    }

    /// Flushes all pending writes to disk.
    #[allow(dead_code)]
    pub(crate) fn flush(&self) -> Result<(), TraceWriterError> {
        self.file.flush()?;
        Ok(())
    }

    /// Flushes and closes the HDF5 file, consuming `self`.
    pub(crate) fn close(self) -> Result<(), TraceWriterError> {
        self.file.flush()?;
        self.file.close()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{digitiser_data::tests::test_internal_structure, handle_trace_message};
    use std::{fs::File, io::Read};

    #[test]
    fn test() {
        let mut file = File::open("test_assets/test.dat2").unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        let mut hdf5 = TraceFileWriter::new_temp("temp").unwrap();
        handle_trace_message(data.as_slice(), Some(&mut hdf5));
        handle_trace_message(data.as_slice(), Some(&mut hdf5));

        assert!(hdf5.digitizers.contains_key(&0));
        test_internal_structure(hdf5.digitizers.get(&0).unwrap());

        let true_hdf5 = hdf5::File::open("test_assets/test.hdf5").unwrap();
        compare_to_true_file(true_hdf5, hdf5.file);
    }

    fn compare_to_true_file(true_hdf5: hdf5::File, test_hdf5: hdf5::File) {
        for true_group in true_hdf5.groups().unwrap() {
            assert!(test_hdf5.group(&true_group.name()).is_ok());
            for true_dataset in true_group.datasets().unwrap() {
                let true_data = true_dataset.read_raw::<u8>().unwrap();

                let test_dataset = test_hdf5.dataset(&true_dataset.name());
                // Ensure test dataset exists.
                assert!(test_dataset.is_ok());

                let test_dataset = test_dataset.unwrap();
                let test_data = test_dataset.read_raw::<u8>();
                // Ensure test dataset value can be read.
                assert!(test_data.is_ok());
                let test_data = test_data.unwrap();

                // Ensure dataset values are equal.
                assert_eq!(true_data, test_data);
            }
        }
    }
}
