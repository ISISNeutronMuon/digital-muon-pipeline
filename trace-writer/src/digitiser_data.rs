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

use crate::error::TraceWriterError;
use chrono::{DateTime, Utc};
use digital_muon_common::Channel;
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::{
    ChannelTrace, DigitizerAnalogTraceMessage,
};
use hdf5::{Dataset, Extent, Group, SimpleExtents};
use ndarray::{Array2, s};
use tracing::warn;

/// Appends a single value to a resizable 1-D HDF5 dataset.
///
/// # Parameters
/// - ds: Dataset to append value, this must be resizable and one dimensional.
/// - value: value to append.
fn append_value<T: hdf5::H5Type>(ds: &Dataset, value: T) -> Result<(), hdf5::Error> {
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

/// Creates a fixed-size 1-D HDF5 dataset of the given type.
///
/// # Parameters
/// - group: the group in which to create the dataset.
/// - name: name of the new dataset.
/// - chunk_size: the chunk size to use.
fn make_fixed_size_dataset<T: hdf5::H5Type>(
    group: &Group,
    name: &str,
    size: usize,
) -> Result<Dataset, hdf5::Error> {
    group
        .new_dataset::<T>()
        .shape(SimpleExtents::fixed(vec![size]))
        .create(name)
}

/// Checks that the existing traces dataset has the appropriate shape for the current message.
/// # Parameters
/// - channels: iterator over the current message's channels.
/// - sizes: the dimensions of the hdf5 traces dataset.
/// - trace_size: the size of the traces of the current message.
///
/// # Return
/// True if everything is valid. false otherwise.
fn validate_current_message_and_trace_sizes<'a>(
    channels: impl ExactSizeIterator<Item = ChannelTrace<'a>> + Clone,
    sizes: &[usize; 3],
    trace_size: usize,
) -> bool {
    if channels.clone().any(|c| c.voltage().is_none()) {
        warn!("Missing channel voltages.");
        return false;
    }
    if channels
        .clone()
        .any(|c| c.voltage().map(|v| v.len() != trace_size).unwrap_or(true))
    {
        warn!("Trace sizes inconsistant.");
        return false;
    }

    if sizes[2] != trace_size {
        warn!(
            "Trace size {trace_size} inconsistant with that of previous message(s) {}.",
            sizes[2]
        );
        return false;
    }
    if sizes[1] != channels.len() {
        warn!(
            "Number of channels {} inconsistant with that of previous message(s) {}.",
            channels.len(),
            sizes[1]
        );
        return false;
    }
    true
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
    /// 1-D resizable dataset containing channel ids.
    ///
    /// Lazily created when the first digitiser message arrives.
    all_channels: Option<Dataset>,
    /// 3-D resizable dataset containing trace data, the shape is [Number of Traces, Number of Channels, Size of Trace].
    ///
    /// Lazily created when the first digitiser message arrives, only the first axis is resized as subsequent messages arrive.
    all_traces: Option<Dataset>,
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

        let frame_number = make_resizable_dataset::<u32>(&group, "frame_number", chunk_size)?;
        let timestamp = make_resizable_dataset::<i64>(&group, "timestamp", chunk_size)?;
        let period_number = make_resizable_dataset::<u64>(&group, "period_number", chunk_size)?;

        Ok(Self {
            group,
            frame_number,
            timestamp,
            period_number,
            all_channels: None,
            all_traces: None,
        })
    }

    /// Lazily creates the `all_channels` and `all_traces` datasets if they do not exist,
    /// using the current message as a template.
    ///
    /// # Parameters
    /// - channels: iterator over the current message's channels.
    /// - chunk_size: the desired chunk size for new datasets.
    /// - trace_size: the size of the traces of the current message.
    fn ensure_channel_and_trace_datasets_exist<'a>(
        &mut self,
        channels: impl ExactSizeIterator<Item = ChannelTrace<'a>> + Clone,
        chunk_size: usize,
        trace_size: usize,
    ) -> Result<(), TraceWriterError> {
        if self.all_channels.is_none() {
            let all_channels =
                make_fixed_size_dataset::<Channel>(&self.group, "channels", channels.len())?;
            all_channels.write(&channels.clone().map(|c| c.channel()).collect::<Vec<_>>())?;
            self.all_channels = Some(all_channels);
        }

        if self.all_traces.is_none() {
            let shape = SimpleExtents::from_vec(vec![
                Extent::resizable(0),
                Extent::fixed(channels.len()),
                Extent::fixed(trace_size),
            ]);
            self.all_traces = Some(
                self.group
                    .new_dataset::<u16>()
                    .shape(shape)
                    .chunk(vec![chunk_size, channels.len(), trace_size])
                    .create("traces")?,
            );
        }
        Ok(())
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

        // Writer frame number and period number.
        append_value(&self.frame_number, metadata.frame_number())?;
        append_value(&self.period_number, metadata.period_number())?;

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

        let trace_size = channels
            .iter()
            .map(|c| c.voltage().map(|v| v.len()).unwrap_or_default())
            .max()
            .unwrap_or_default();

        self.ensure_channel_and_trace_datasets_exist(channels.iter(), chunk_size, trace_size)?;
        let all_traces = self.all_traces.as_ref().expect("This should never fail.");
        let all_traces_sizes: [usize; 3] = all_traces
            .shape()
            .try_into()
            .expect("Dataset should have three dimensions, this should never fail");

        if !validate_current_message_and_trace_sizes(channels.iter(), &all_traces_sizes, trace_size)
        {
            return Ok(());
        }

        // Extend the traces field in the first axis.
        let new_sizes = {
            let mut new_sizes = all_traces_sizes;
            new_sizes[0] += 1;
            new_sizes
        };
        all_traces.resize(new_sizes)?;

        // Build the next slice of the traces field.
        let traces = channels
            .iter()
            .flat_map(|channel_trace| {
                channel_trace
                    .voltage()
                    .expect("Voltage field should exist, this should never fail.")
                    .iter()
            })
            .collect::<Vec<_>>();
        let traces = Array2::from_shape_vec([all_traces_sizes[1], all_traces_sizes[2]], traces)
            .expect("Traces slice should have the correct size, this should never fail.");
        let slice = s![
            all_traces_sizes[0],
            0..all_traces_sizes[1],
            0..all_traces_sizes[2]
        ];
        all_traces.write_slice(&traces, slice)?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use digital_muon_common::{FrameNumber, Intensity};
    use ndarray::Dim;

    pub(crate) fn test_internal_structure(digitiser: &DigitizerData) {
        let frame_number = digitiser.frame_number.read_1d::<FrameNumber>();
        let period_number = digitiser.period_number.read_1d::<u64>();
        let timestamp = digitiser.timestamp.read_1d::<i64>();
        assert!(frame_number.is_ok());
        assert!(period_number.is_ok());
        assert!(timestamp.is_ok());
        assert_eq!(frame_number.unwrap().to_vec(), vec![0]);
        assert_eq!(period_number.unwrap().to_vec(), vec![0]);
        assert_eq!(timestamp.unwrap().to_vec(), vec![1781869468296159408]);

        assert!(digitiser.all_channels.is_some());
        let channels = digitiser
            .all_channels
            .as_ref()
            .unwrap()
            .read_1d::<Channel>();
        assert!(channels.is_ok());
        assert_eq!(channels.unwrap().to_vec(), vec![0, 1, 2, 3, 4, 5, 6, 7]);
        assert!(digitiser.all_traces.is_some());
        let traces = digitiser
            .all_traces
            .as_ref()
            .unwrap()
            .read::<Intensity, Dim<[usize; 3]>>();
        assert!(traces.is_ok());
        assert_eq!(traces.unwrap().shape(), &[1, 8, 50]);
    }
}
