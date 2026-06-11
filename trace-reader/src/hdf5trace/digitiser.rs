use chrono::{DateTime, Duration, Utc};
use digital_muon_common::{Channel, DigitizerId, FrameNumber};
use digital_muon_streaming_types::{
    dat2_digitizer_analog_trace_v2_generated::{
        DigitizerAnalogTraceMessage, DigitizerAnalogTraceMessageArgs,
        finish_digitizer_analog_trace_message_buffer,
    },
    flatbuffers::{FlatBufferBuilder, WIPOffset},
    frame_metadata_v2_generated::{FrameMetadataV2, FrameMetadataV2Args, GpsTime},
};
use hdf5::{File, types::VarLenUnicode};

use crate::hdf5trace::{
    cached_dataset::CachedDataset, channel::Hdf5Channel, extract_from_dataset_name,
};

const CACHE_SIZE: Option<usize> = Some(30);

const CHANNEL: &'static str = "channel";
const DIGITISER: &'static str = "digitiser";

pub(crate) struct Hdf5Digitiser {
    digitiser_id: DigitizerId,
    period_numbers: CachedDataset<u64>,
    frame_numbers: CachedDataset<FrameNumber>,
    timestamps: CachedDataset<VarLenUnicode>,
    channels: Vec<Hdf5Channel>,
    num_frames: usize,
}

impl Hdf5Digitiser {
    pub(crate) fn open_from(file: File) -> Result<Vec<Hdf5Digitiser>, String> {
        let mut digitisers = Vec::<Hdf5Digitiser>::new();
        for group in file.groups().unwrap() {
            let digitiser_id: DigitizerId = extract_from_dataset_name(group.name(), DIGITISER)?;

            let frame_numbers =
                CachedDataset::new(group.dataset("frame_number").unwrap(), CACHE_SIZE);
            let period_numbers =
                CachedDataset::new(group.dataset("period_number").unwrap(), CACHE_SIZE);
            let timestamps = CachedDataset::new(group.dataset("timestamp").unwrap(), CACHE_SIZE);

            let num_frames = frame_numbers.get_num_element();
            assert_eq!(period_numbers.get_num_element(), num_frames);
            assert_eq!(timestamps.get_num_element(), num_frames);

            let mut channels = Vec::<Hdf5Channel>::new();

            for dataset in group.datasets().unwrap() {
                let name = dataset.name();
                if ["frame_number", "period_number", "timestamp"]
                    .contains(&name.split('/').last().unwrap())
                {
                    break;
                }

                let channel: Channel = extract_from_dataset_name(name, CHANNEL)?;
                assert_eq!(*dataset.shape().get(0).unwrap(), num_frames);
                channels.push(Hdf5Channel::new(
                    channel,
                    CachedDataset::new(dataset, CACHE_SIZE),
                ));
            }
            digitisers.push(Hdf5Digitiser {
                digitiser_id,
                period_numbers,
                frame_numbers,
                timestamps,
                channels,
                num_frames,
            });
        }
        Ok(digitisers)
    }

    pub(crate) fn ensure_elements_cached(&mut self, index: usize) {
        self.period_numbers.ensure_elements_cached(index);
        self.timestamps.ensure_elements_cached(index);
        self.frame_numbers.ensure_elements_cached(index);
        for channel in &mut self.channels {
            channel.ensure_elements_cached(index);
        }
    }
    pub(crate) fn get_num_frames(&self) -> usize {
        self.num_frames
    }

    /// Loads a FlatBufferBuilder with a new DigitizerAnalogTraceMessage instance with a custom timestamp.
    /// #Arguments
    /// * `fbb` - A mutable reference to the FlatBufferBuilder to use.
    /// * `time` - A `frame_metadata_v2_generated::GpsTime` instance containing the timestamp.
    /// * `frame_number` - The frame number to use.
    /// * `digitizer_id` - The id of the digitizer to use.
    /// * `measurements_per_frame` - The number of measurements to simulate in each channel.
    /// * `num_channels` - The number of channels to simulate.
    ///
    /// #Returns
    /// A string result, or an error.
    pub(crate) fn create_message(
        &self,
        fbb: &mut FlatBufferBuilder<'_>,
        index: usize,
        sampling_rate: u64,
    ) -> miette::Result<()> {
        if index > self.num_frames {
            return Ok(());
        }
        fbb.reset();

        let frame_number = *self.frame_numbers.get_element(index);
        let period_number = *self.period_numbers.get_element(index);
        let timestamp: DateTime<Utc> = self.timestamps.get_element(index).parse().unwrap();
        let timestamp = timestamp + Duration::days(2);

        let channels = self
            .channels
            .iter()
            .map(|c| c.create_channel(fbb, index))
            .collect::<Vec<_>>();

        let gps_time = GpsTime::from(timestamp);
        let metadata: FrameMetadataV2Args = FrameMetadataV2Args {
            frame_number,
            period_number,
            protons_per_pulse: 0,
            running: true,
            timestamp: Some(&gps_time),
            veto_flags: 0,
        };
        let metadata: WIPOffset<FrameMetadataV2> = FrameMetadataV2::create(fbb, &metadata);

        let message = DigitizerAnalogTraceMessageArgs {
            digitizer_id: self.digitiser_id,
            metadata: Some(metadata),
            sample_rate: sampling_rate,
            channels: Some(fbb.create_vector_from_iter(channels.iter())),
        };
        let message = DigitizerAnalogTraceMessage::create(fbb, &message);
        finish_digitizer_analog_trace_message_buffer(fbb, message);
        Ok(())
    }
}
