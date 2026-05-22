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
//!     timestamp     : [VarLenUnicode]    – ISO-8601 timestamp per message
//!     period_number : [u64]              – one entry per received message
//!     sample_rate   : [u64]              – sample rate (Hz) per message
//!     channel_{n}   : [VarLenArray<u16>] – voltage trace per message (variable length)
//! ```

use crate::error::TraceWriterError;
use chrono::{DateTime, Utc};
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::DigitizerAnalogTraceMessage;
use hdf5::{
    Dataset, File, Group, SimpleExtents,
    types::{VarLenArray, VarLenUnicode},
};
use ndarray::s;
use std::{collections::HashMap, path::Path};

/// Format used to serialise GPS timestamps as strings in the HDF5 file.
const DATETIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.9f%:z";

// ─── low-level helpers ────────────────────────────────────────────────────────

/// Appends a single value to a resizable 1-D HDF5 dataset.
fn append_value<T: hdf5::H5Type>(ds: &Dataset, value: T) -> Result<(), hdf5::Error> {
    let cur = ds.size();
    ds.resize(cur + 1)?;
    ds.write_slice(&[value], s![cur..cur + 1])?;
    Ok(())
}

/// Creates a resizable 1-D HDF5 dataset of the given type.
fn make_resizable_dataset<T: hdf5::H5Type>(
    group: &Group,
    name: &str,
    chunk_size: usize,
) -> Result<Dataset, hdf5::Error> {
    group
        .new_dataset::<T>()
        .shape(SimpleExtents::resizable(vec![0]))
        .chunk(vec![chunk_size])
        .create(name)
}

// ─── per-digitiser data ───────────────────────────────────────────────────────

/// Owns the HDF5 group and datasets for one digitiser.
struct DigitizerData {
    /// HDF5 group for this digitiser.
    group: Group,
    /// 1-D resizable dataset: frame number per message.
    frame_number: Dataset,
    /// 1-D resizable dataset: ISO timestamp string per message.
    timestamp: Dataset,
    /// 1-D resizable dataset: period number per message.
    period_number: Dataset,
    /// 1-D resizable dataset: sample rate per message.
    sample_rate: Dataset,
    /// Per-channel voltage datasets, keyed by channel number.
    channels: HashMap<u32, Dataset>,
}

impl DigitizerData {
    /// Creates the HDF5 group and metadata datasets for a new digitiser.
    fn new(parent: &Group, digitizer_id: u8, chunk_size: usize) -> Result<Self, hdf5::Error> {
        let group = parent.create_group(&format!("digitiser_{digitizer_id}"))?;

        let frame_number = make_resizable_dataset::<u32>(&group, "frame_number", chunk_size)?;
        let timestamp = make_resizable_dataset::<VarLenUnicode>(&group, "timestamp", chunk_size)?;
        let period_number = make_resizable_dataset::<u64>(&group, "period_number", chunk_size)?;
        let sample_rate = make_resizable_dataset::<u64>(&group, "sample_rate", chunk_size)?;

        Ok(Self {
            group,
            frame_number,
            timestamp,
            period_number,
            sample_rate,
            channels: HashMap::new(),
        })
    }

    /// Writes one [DigitizerAnalogTraceMessage] into this digitiser's datasets.
    fn write_trace(
        &mut self,
        msg: &DigitizerAnalogTraceMessage<'_>,
        chunk_size: usize,
    ) -> Result<(), TraceWriterError> {
        let metadata = msg.metadata();

        // ── metadata fields ──────────────────────────────────────────────────
        append_value(&self.frame_number, metadata.frame_number())?;
        append_value(&self.period_number, metadata.period_number())?;
        append_value(&self.sample_rate, msg.sample_rate())?;

        // ── timestamp ────────────────────────────────────────────────────────
        let gps = metadata
            .timestamp()
            .copied()
            .ok_or(TraceWriterError::MissingField("timestamp"))?;
        let dt: DateTime<Utc> = gps
            .try_into()
            .map_err(|_| TraceWriterError::TimestampConversionFailed)?;
        let ts_str = dt.format(DATETIME_FORMAT).to_string();
        let vlu: VarLenUnicode = ts_str
            .parse()
            .map_err(|_| TraceWriterError::UnicodeConversionFailed(ts_str.clone()))?;
        append_value(&self.timestamp, vlu)?;

        // ── channel voltage traces ────────────────────────────────────────────
        let Some(channels) = msg.channels() else {
            return Ok(());
        };

        for channel_trace in channels.iter() {
            let channel_num = channel_trace.channel();

            // Lazily create a dataset for this channel.
            if !self.channels.contains_key(&channel_num) {
                let ds = make_resizable_dataset::<VarLenArray<u16>>(
                    &self.group,
                    &format!("channel_{channel_num}"),
                    chunk_size,
                )?;
                self.channels.insert(channel_num, ds);
            }
            let ds = self
                .channels
                .get(&channel_num)
                .expect("channel was just inserted");

            let voltage: Vec<u16> = channel_trace
                .voltage()
                .map(|v| v.iter().collect())
                .unwrap_or_default();
            let vla = VarLenArray::from_slice(&voltage);
            append_value(ds, vla)?;
        }

        Ok(())
    }
}

// ─── public API ──────────────────────────────────────────────────────────────

/// Manages one open HDF5 file for writing digitiser trace data.
///
/// Digitiser groups and channel datasets are created lazily the first time
/// a message from that digitiser / channel is received.
pub(crate) struct TraceFileWriter {
    file: File,
    chunk_size: usize,
    digitizers: HashMap<u8, DigitizerData>,
}

impl TraceFileWriter {
    /// Creates a new HDF5 file at `path` and prepares it for writing.
    pub(crate) fn new(path: &Path, chunk_size: usize) -> Result<Self, TraceWriterError> {
        let file = File::create(path)?;
        Ok(Self {
            file,
            chunk_size,
            digitizers: HashMap::new(),
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

        let chunk_size = self.chunk_size;
        let dig_data = self
            .digitizers
            .get_mut(&digitizer_id)
            .expect("digitizer was just inserted");
        dig_data.write_trace(msg, chunk_size)?;

        Ok(())
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
