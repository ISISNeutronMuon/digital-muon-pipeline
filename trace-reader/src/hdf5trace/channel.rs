use digital_muon_common::{Channel, Intensity};
use digital_muon_streaming_types::{
    dat2_digitizer_analog_trace_v2_generated::{ChannelTrace, ChannelTraceArgs},
    flatbuffers::{FlatBufferBuilder, ForwardsUOffset, Vector, WIPOffset},
};
use hdf5::{Dataset, types::VarLenArray};
use ndarray::{Dim, IxDynImpl};

use crate::hdf5trace::cached_dataset::{CachedDataset, FullDataset};

pub(super) struct Hdf5Channel {
    channel: Channel,
    traces: CachedDataset<VarLenArray<Intensity>>,
}

impl Hdf5Channel {
    pub(super) fn new(channel: Channel, traces: CachedDataset<VarLenArray<Intensity>>) -> Self {
        Self { channel, traces }
    }

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

    #[tracing::instrument(skip_all, fields(channel = self.channel))]
    pub(super) fn ensure_elements_cached(&mut self, index: usize) {
        self.traces.ensure_elements_cached(index);
    }
}


pub(super) struct Hdf5AllChannels {
    channels: FullDataset<Channel>,
    traces: Dataset,
}

impl Hdf5AllChannels {
    pub(super) fn new(channels: FullDataset<Channel>, traces: Dataset) -> Self {
        Self { channels, traces }
    }

    #[tracing::instrument(skip_all, fields(index = index, length))]
    pub(super) fn create_channels<'a>(
        &self,
        fbb: &mut FlatBufferBuilder<'a>,
        index: usize,
    ) -> WIPOffset<Vector<'a, ForwardsUOffset<ChannelTrace<'a>>>> {
        let trace = self.traces.read_slice_2d::<u16,_>(ndarray::s![..,..,index]).unwrap();
        
        /*let traces = self.channels.iter().map(|&channel| {
            tracing::Span::current().record("length", trace.len());
            let voltage = Some(fbb.create_vector::<Intensity>(trace.as_slice()));
            ChannelTrace::create(
                fbb,
                &ChannelTraceArgs {
                    channel,
                    voltage,
                },
            )
        });*/
        //.collect::<Vec<_>>();
        tracing::Span::current().record("length", self.channels.get_num_elements());
        let traces = self.channels.iter().enumerate().map(|(index, &channel)| {
            let slice = trace.slice(ndarray::s![..,index]);
            let v = slice.into_iter().copied().collect::<Vec<_>>();
            let voltage = Some(fbb.create_vector::<Intensity>(v.as_slice()));
            ChannelTrace::create(
                fbb,
                &ChannelTraceArgs {
                    channel,
                    voltage,
                },
            )
        }).collect::<Vec<_>>();
        fbb.create_vector::<WIPOffset<ChannelTrace>>(traces.as_slice())
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn ensure_elements_cached(&mut self, index: usize) {
        //self.traces.ensure_elements_cached(index);
    }
}
