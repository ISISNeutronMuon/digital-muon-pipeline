use digital_muon_common::{Channel, Intensity};
use digital_muon_streaming_types::{
    dat2_digitizer_analog_trace_v2_generated::{ChannelTrace, ChannelTraceArgs},
    flatbuffers::{FlatBufferBuilder, WIPOffset},
};
use hdf5::types::VarLenArray;

use crate::hdf5trace::cached_dataset::CachedDataset;

pub(super) struct Hdf5Channel {
    channel: Channel,
    traces: CachedDataset<VarLenArray<Intensity>>,
}

impl Hdf5Channel {
    pub(super) fn new(channel: Channel, traces: CachedDataset<VarLenArray<Intensity>>) -> Self {
        Self { channel, traces }
    }

    pub(super) fn create_channel<'a>(
        &self,
        fbb: &mut FlatBufferBuilder<'a>,
        index: usize,
    ) -> WIPOffset<ChannelTrace<'a>> {
        let trace = self.traces.get_element(index);
        let voltage = Some(fbb.create_vector::<Intensity>(trace.as_slice()));
        ChannelTrace::create(
            fbb,
            &ChannelTraceArgs {
                channel: self.channel,
                voltage,
            },
        )
    }

    pub(super) fn ensure_elements_cached(&mut self, index: usize) {
        self.traces.ensure_elements_cached(index);
    }
}
