use crate::{
    digitiser_data::{append_value, make_resizable_dataset},
    error::TraceWriterError,
};
use digital_muon_common::Channel;
use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::ChannelTrace;
use hdf5::{Dataset, Extent, Group, SimpleExtents};
use itertools::Itertools;
use ndarray::s;
#[cfg(test)]
use ndarray::{ArrayBase, Dim, OwnedRepr};
use tracing::warn;

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

/// Encapsulates the sizes of a digitiser trace message.
struct TraceSizes {
    /// Number of time bins.
    trace_size: usize,
    /// Number of channels in message.
    number_of_channels: usize,
}

/// Owns the HDF5 group and datasets for the trace data.
pub(crate) struct TraceData {
    /// 2-D dataset, resizable in the first dimension, containing trace data, the shape is [Number of Channels, Total Size of Traces].
    all_traces: Dataset,
    /// 1-D resizable dataset whose ith value is the position of the ith trace in `all_traces`.
    trace_index: Dataset,
    /// The position in the second dimension of `all_traces` to write the next message of trace data.
    next_trace_index: usize,
    /// If set, encapsulates the sizes of the previous trace message.
    previous_trace: Option<TraceSizes>,
}

impl TraceData {
    /// Creates the datasets for the channel ids and trace data.
    ///
    /// # Parameters
    /// - group: the parent group to use.
    /// - channels: iterator over the current message's channels.
    /// - chunk_size: the desired chunk size for new datasets.
    pub(crate) fn new<'a>(
        group: &Group,
        channels: impl ExactSizeIterator<Item = ChannelTrace<'a>> + Clone,
        chunk_size: usize,
    ) -> Result<Self, hdf5::Error> {
        make_fixed_size_dataset::<Channel>(group, "channels", channels.len())?
            .write(&channels.clone().map(|c| c.channel()).collect::<Vec<_>>())?;

        let shape =
            SimpleExtents::from_vec(vec![Extent::fixed(channels.len()), Extent::resizable(0)]);
        let all_traces = group
            .new_dataset::<u16>()
            .shape(shape)
            .chunk(vec![channels.len(), chunk_size])
            .create("traces")?;
        let trace_index = make_resizable_dataset::<usize>(group, "trace_index", chunk_size)?;
        Ok(Self {
            all_traces,
            trace_index,
            previous_trace: None,
            next_trace_index: 0,
        })
    }

    /// Checks that the existing traces dataset has the appropriate shape for the current message.
    /// # Parameters
    /// - channels: iterator over the current message's channels.
    ///
    /// # Return
    /// The current trace size if everything is valid, `None` otherwise.
    fn validate_current_message_and_get_current_trace_size<'a>(
        &self,
        channels: impl ExactSizeIterator<Item = ChannelTrace<'a>> + Clone,
    ) -> Option<TraceSizes> {
        if channels.len() == 0 {
            warn!("Missing channels.");
            return None;
        }
        if channels.clone().any(|c| c.voltage().is_none()) {
            warn!("Missing channel voltages.");
            return None;
        }

        // This reduces the channels down to the vector of unique lengths.
        // In normal operation, there should be only one unique length.
        let lengths = channels
            .clone()
            .flat_map(|c| c.voltage().map(|c| c.len()))
            .unique()
            .collect::<Vec<_>>();
        // If the trace lengths are not unique then return `None`.
        if lengths.len() != 1 {
            warn!("Trace sizes inconsistant {lengths:?}.");
            return None;
        }

        let current_trace_size = *lengths
            .first()
            .expect("Lengths should be non-empty, this should never fail.");
        let current_num_channels = channels.len();
        if let Some(previous_trace) = self.previous_trace.as_ref() {
            if current_trace_size != previous_trace.trace_size {
                warn!(
                    "Trace size: {current_trace_size} inconsistant with that of previous message: {}.",
                    previous_trace.trace_size
                );
            }

            if previous_trace.number_of_channels != current_num_channels {
                warn!(
                    "Number of Channels: {current_num_channels} inconsistant with that of previous message: {}.",
                    previous_trace.number_of_channels
                );
            }
        }
        Some(TraceSizes {
            trace_size: current_trace_size,
            number_of_channels: current_num_channels,
        })
    }

    /// Creates the datasets for the channel ids and trace data,
    /// and updates the internal state.
    ///
    /// # Parameters
    /// - channels: iterator over the current message's channels.
    pub(crate) fn write_trace<'a>(
        &mut self,
        channels: impl ExactSizeIterator<Item = ChannelTrace<'a>> + Clone,
    ) -> Result<(), TraceWriterError> {
        let Some(current_trace) =
            self.validate_current_message_and_get_current_trace_size(channels.clone())
        else {
            return Ok(());
        };

        self.write_trace_inner(channels.clone(), current_trace.trace_size)?;

        self.next_trace_index += current_trace.trace_size;
        self.previous_trace = Some(current_trace);
        Ok(())
    }

    /// Creates the datasets for the channel ids and trace data.
    ///
    /// # Parameters
    /// - channels: iterator over the current message's channels.
    /// - current_trace_size: the size of the traces of the current message.
    fn write_trace_inner<'a>(
        &self,
        channels: impl ExactSizeIterator<Item = ChannelTrace<'a>> + Clone,
        current_trace_size: usize,
    ) -> Result<(), TraceWriterError> {
        let all_traces_sizes: [usize; 2] = self
            .all_traces
            .shape()
            .try_into()
            .expect("Dataset should have two dimensions, this should never fail");

        // Extend the traces field in the first axis.
        let new_sizes = {
            let mut new_sizes = all_traces_sizes;
            new_sizes[1] += current_trace_size;
            new_sizes
        };
        self.all_traces.resize(new_sizes)?;

        for (idx, channel) in channels.enumerate() {
            let slice = s![
                idx,
                self.next_trace_index..(self.next_trace_index + current_trace_size)
            ];
            let trace = channel
                .voltage()
                .expect("Voltage field should exist, this should never fail.")
                .into_iter()
                .collect::<Vec<_>>();
            self.all_traces.write_slice(&trace, slice)?;
        }

        append_value(&self.trace_index, self.next_trace_index)?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn read(&self) -> Result<ReadType, hdf5::Error> {
        self.all_traces.read::<_, _>()
    }
}

#[cfg(test)]
type ReadType = ArrayBase<OwnedRepr<u16>, Dim<[usize; 2]>, u16>;
