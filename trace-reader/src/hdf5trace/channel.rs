use crate::hdf5trace::cached_dataset::CachedDataset;
use digital_muon_common::{Channel, Intensity};
use digital_muon_streaming_types::{
    dat2_digitizer_analog_trace_v2_generated::{ChannelTrace, ChannelTraceArgs},
    flatbuffers::{FlatBufferBuilder, ForwardsUOffset, Vector, WIPOffset},
};
use hdf5::{Dataset, types::VarLenArray};
use ndarray::Array1;

/// Encapsulates the hdf5 groups of a single channel group.
pub(super) struct Hdf5Channel {
    /// The channel id.
    channel: Channel,
    /// The trace data, stored in the file as an array of arrays.
    traces: CachedDataset<VarLenArray<Intensity>>,
}

impl Hdf5Channel {
    /// Creates a new instance from the given id and traces object.
    pub(super) fn new(channel: Channel, traces: CachedDataset<VarLenArray<Intensity>>) -> Self {
        Self { channel, traces }
    }

    /// Create the FlatBuffer structure of the channel data for the given index.
    ///
    /// # Parameters
    /// - fbb: mutable reference to the FlatBufferBuilder to use.
    /// - index: the index of the trace to use.
    #[tracing::instrument(skip_all, fields(index = index, channel = self.channel, length))]
    pub(super) fn create_channel<'a>(
        &self,
        fbb: &mut FlatBufferBuilder<'a>,
        index: usize,
    ) -> WIPOffset<ChannelTrace<'a>> {
        let trace = self.traces.get_element(index);
        tracing::Span::current().record("length", trace.len());
        let voltage = Some(fbb.create_vector::<Intensity>(trace.as_slice()));
        ChannelTrace::create(
            fbb,
            &ChannelTraceArgs {
                channel: self.channel,
                voltage,
            },
        )
    }

    /// Given an index, ensure the necessary data is in the cache.
    /// This should each time before the `create_channel` method is used.
    ///
    /// This method is idempotent, so does nothing if the required index is already cached.
    ///
    /// # Parameters
    /// - index: the index to ensure is cached.
    #[tracing::instrument(skip_all, fields(channel = self.channel))]
    pub(super) fn ensure_elements_cached(&mut self, index: usize) {
        self.traces.ensure_elements_cached(index);
    }
}

/// Encapsulates the hdf5 data when all channel's trace data are stored in a single dataset.
pub(super) struct Hdf5AllChannels {
    /// Array of channel ids.
    channels: Array1<Channel>,
    /// Three-dimensional dataset containing trace data, the shape is [Number of Traces, Number of Channels, Size of Trace].
    traces: Dataset,
}

impl Hdf5AllChannels {
    /// Creates a new instance from the given id and traces object.
    pub(super) fn new(channels: Array1<Channel>, traces: Dataset) -> Self {
        Self { channels, traces }
    }

    /// Create the FlatBuffer structure of the channel data for the given index.
    ///
    /// # Parameters
    /// - fbb: mutable reference to the FlatBufferBuilder to use.
    /// - index: the index of the trace to use.
    #[tracing::instrument(skip_all, fields(index = index, length))]
    pub(super) fn create_channels<'a>(
        &self,
        fbb: &mut FlatBufferBuilder<'a>,
        index: usize,
    ) -> WIPOffset<Vector<'a, ForwardsUOffset<ChannelTrace<'a>>>> {
        tracing::Span::current().record("length", self.channels.len());
        let trace = self
            .traces
            .read_slice_2d::<u16, _>(ndarray::s![index, .., ..])
            .expect("2D Slice should be present in trace dataset. This should never fail.");
        let traces =
            self.channels
                .iter()
                .enumerate()
                .map(|(index, &channel)| {
                    let slice = trace.slice(ndarray::s![index, ..]);
                    let voltage = Some(fbb.create_vector::<Intensity>(slice.as_slice().expect(
                        "Should be able to coerce to slice type. This should never fail.",
                    )));
                    ChannelTrace::create(fbb, &ChannelTraceArgs { channel, voltage })
                })
                .collect::<Vec<_>>();
        fbb.create_vector::<WIPOffset<ChannelTrace>>(traces.as_slice())
    }
}
