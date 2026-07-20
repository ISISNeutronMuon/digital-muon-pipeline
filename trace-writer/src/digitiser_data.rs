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
//!     timestamp     : [i64]              – timestamp nanoseconds since epoch per message
//!     period_number : [u64]              – one entry per received message
//!     trace_index   : [usize]            - one entry per received message
//!     all_traces    : [usize, usize]     – voltage traces per message (variable length)
//! ```

use crate::{error::TraceWriterError, trace_data::TraceData};
use chrono::{DateTime, Utc};
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::DigitizerAnalogTraceMessage;
use hdf5::{Dataset, Group, SimpleExtents};
use ndarray::s;
use tracing::warn;

/// Appends a single value to a resizable 1-D HDF5 dataset.
///
/// # Parameters
/// - ds: Dataset to append value, this must be resizable and one dimensional.
/// - value: value to append.
pub(crate) fn append_value<T: hdf5::H5Type>(ds: &Dataset, value: T) -> Result<(), hdf5::Error> {
    let cur = ds.size();
    ds.resize(cur + 1)?;
    ds.write_slice(&[value], s![cur..cur + 1])?;
    Ok(())
}

/// Creates a resizable 1-D HDF5 dataset of the given type.
///
/// # Parameters
/// - group: the group in which to create the dataset.
/// - name: name of the new dataset.
/// - chunk_size: the chunk size to use.
pub(crate) fn make_resizable_dataset<T: hdf5::H5Type>(
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

/// Owns the HDF5 group and datasets for one digitiser.
pub(crate) struct DigitizerData {
    /// HDF5 group for this digitiser.
    group: Group,
    /// 1-D resizable dataset: frame number per message.
    frame_number: Dataset,
    /// 1-D resizable dataset: RFC3339 timestamp string per message.
    timestamp: Dataset,
    /// 1-D resizable dataset: period number per message.
    period_number: Dataset,
    /// 1-D resizable dataset: protons per pulse per message.
    protons_per_pulse: Dataset,
    /// 1-D resizable dataset: running flag per message.
    running: Dataset,
    /// 1-D resizable dataset: veto flags per message.
    veto_flags: Dataset,
    /// 1-D resizable dataset: sample rate per message.
    sample_rate: Dataset,
    /// Dataset containing channel ids and trace data.
    ///
    /// Lazily created when the first digitiser message arrives.
    traces: Option<TraceData>,
}

impl DigitizerData {
    /// Creates the HDF5 group and metadata datasets for a new digitiser.
    ///
    /// # Parameters
    /// - parent: the group in which to create the digitiser group.
    /// - digitizer_id: the id of the digitiser.
    /// - chunk_size: the chunk size to use.
    pub(crate) fn new(
        parent: &Group,
        digitizer_id: u8,
        chunk_size: usize,
    ) -> Result<Self, hdf5::Error> {
        let group = parent.create_group(&format!("digitiser_{digitizer_id}"))?;

        let sample_rate = make_resizable_dataset::<u64>(&group, "sample_rate", chunk_size)?;
        let frame_number = make_resizable_dataset::<u32>(&group, "frame_number", chunk_size)?;
        let timestamp = make_resizable_dataset::<i64>(&group, "timestamp", chunk_size)?;
        let period_number = make_resizable_dataset::<u64>(&group, "period_number", chunk_size)?;
        let protons_per_pulse =
            make_resizable_dataset::<u8>(&group, "protons_per_pulse", chunk_size)?;
        let running = make_resizable_dataset::<bool>(&group, "running", chunk_size)?;
        let veto_flags = make_resizable_dataset::<u16>(&group, "veto_flags", chunk_size)?;

        Ok(Self {
            group,
            frame_number,
            timestamp,
            period_number,
            sample_rate,
            protons_per_pulse,
            running,
            veto_flags,
            traces: None,
        })
    }

    /// Writes one [DigitizerAnalogTraceMessage] into this digitiser's datasets.
    ///
    /// # Parameters
    /// - msg: the trace message to write.
    /// - chunk_size: chunk size for new datasets.
    pub(crate) fn write_trace(
        &mut self,
        msg: &DigitizerAnalogTraceMessage<'_>,
        chunk_size: usize,
    ) -> Result<(), TraceWriterError> {
        let metadata = msg.metadata();

        // Write metadata and sample rate.
        append_value(&self.frame_number, metadata.frame_number())?;
        append_value(&self.period_number, metadata.period_number())?;
        append_value(&self.protons_per_pulse, metadata.protons_per_pulse())?;
        append_value(&self.running, metadata.running())?;
        append_value(&self.veto_flags, metadata.veto_flags())?;
        append_value(&self.sample_rate, msg.sample_rate())?;

        // Extract timestamp and write.
        let timestamp: DateTime<Utc> = metadata
            .timestamp()
            .copied()
            .ok_or(TraceWriterError::MissingField("timestamp"))?
            .try_into()
            .map_err(|_| TraceWriterError::TimestampConversionFailed)?;
        let timestamp_ns = timestamp
            .timestamp_nanos_opt()
            .ok_or(TraceWriterError::NanosecondConversionFailed(timestamp))?;
        append_value(&self.timestamp, timestamp_ns)?;

        // Extract channels from current message, returning a warning if not present.
        let Some(channels) = msg.channels() else {
            warn!("Channels field missing.");
            return Ok(());
        };

        // Lazily creates the `traces` structure if they do not exist, using the current message as a template.
        if self.traces.is_none() {
            self.traces = Some(TraceData::new(&self.group, channels.iter(), chunk_size)?);
        }
        let all_traces = self.traces.as_mut().expect("This should never fail.");
        all_traces.write_trace(channels.iter())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use digital_muon_common::{Channel, FrameNumber};

    pub(crate) fn test_internal_structure(digitiser: &DigitizerData) {
        let frame_number = digitiser.frame_number.read_1d::<FrameNumber>();
        let period_number = digitiser.period_number.read_1d::<u64>();
        let timestamp = digitiser.timestamp.read_1d::<i64>();
        assert!(frame_number.is_ok());
        assert!(period_number.is_ok());
        assert!(timestamp.is_ok());
        assert_eq!(frame_number.unwrap().to_vec(), vec![0, 0]);
        assert_eq!(period_number.unwrap().to_vec(), vec![0, 0]);
        assert_eq!(
            timestamp.unwrap().to_vec(),
            vec![1781869468296159408, 1781869468296159408]
        );

        assert!(digitiser.traces.is_some());
        assert!(digitiser.group.dataset("channels").is_ok());
        let channels = digitiser
            .group
            .dataset("channels")
            .unwrap()
            .read_1d::<Channel>();
        assert!(channels.is_ok());
        assert_eq!(channels.unwrap().to_vec(), vec![0, 1, 2, 3, 4, 5, 6, 7]);
        assert!(digitiser.traces.is_some());
        let traces = digitiser.traces.as_ref().unwrap().read();
        assert!(traces.is_ok());
        assert_eq!(traces.unwrap().shape(), &[8, 100]);
    }
}
