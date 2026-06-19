use chrono::{DateTime, Datelike, Utc};
use digital_muon_common::{Channel, DigitizerId, FrameNumber};
use digital_muon_streaming_types::{
    dat2_digitizer_analog_trace_v2_generated::{
        DigitizerAnalogTraceMessage, DigitizerAnalogTraceMessageArgs,
        finish_digitizer_analog_trace_message_buffer,
    },
    flatbuffers::{FlatBufferBuilder, WIPOffset},
    frame_metadata_v2_generated::{FrameMetadataV2, FrameMetadataV2Args, GpsTime},
};
use hdf5::{File, Group, types::VarLenUnicode};
use ndarray::Array1;
use tracing::{info, warn};

use crate::hdf5trace::{
    Error, cached_dataset::CachedDataset, channel::{Hdf5AllChannels, Hdf5Channel}, extract_from_dataset_name
};

const CHANNEL: &'static str = "channel";
const DIGITISER: &'static str = "digitiser";

#[derive(Default, Debug)]
pub(crate) struct HDF5Config {
    /// True if the timestamp is stored as a RFC3339 string, otherwise ns since epoch.
    pub(crate) timestamp_as_rfc3339: bool,
    /// True if channels are stored in separate datasets, false if channels are stored as 2D array.
    pub(crate) multiple_channel_datasets: bool,
    /// 
    pub(crate) cache_size: Option<usize>,
}

enum Timestamps {
    RFC3999(CachedDataset<VarLenUnicode>),
    EPOCHNS(Array1<i64>)
}

enum Channels {
    MULTIPLE(Vec<Hdf5Channel>),
    SINGLE(Hdf5AllChannels)
}

pub(crate) struct Hdf5Digitiser {
    digitiser_id: DigitizerId,
    period_numbers: Array1<u64>,
    frame_numbers: Array1<FrameNumber>,
    timestamps: Timestamps,
    channels: Channels,
    num_frames: usize,
}

impl Hdf5Digitiser {
    pub(crate) fn open_from(file: File, config: HDF5Config) -> Result<Vec<Hdf5Digitiser>, Error> {
        let mut digitisers = Vec::<Hdf5Digitiser>::new();
        for group in file.groups().expect("Groups should be accessible, this should never fail.") {
            digitisers.push(Self::open_digitiser(group, &config)?);
        }
        Ok(digitisers)
    }

    fn open_digitiser(group: Group, config: &HDF5Config) -> Result<Self,Error> {
        let digitiser_id: DigitizerId = extract_from_dataset_name(group.name(), DIGITISER)?;

        let frame_numbers = group
            .dataset("frame_number")?
            .read_1d()?;
        let num_frames = frame_numbers.len();

        let period_numbers = group
            .dataset("period_number")?
            .read_1d()?;
        assert_eq!(period_numbers.len(), num_frames);

        let timestamps = if config.timestamp_as_rfc3339 {
            let timestamps = CachedDataset::new(group.dataset("timestamp")?, config.cache_size.as_ref())?;
            assert_eq!(timestamps.get_num_elements(), num_frames);
            Timestamps::RFC3999(timestamps)
        } else {
            let timestamps : Array1<_> = group
                .dataset("timestamp")?
                .read_1d()?;
            assert_eq!(timestamps.len(), num_frames);
            Timestamps::EPOCHNS(timestamps)
        };

        let channels = if config.multiple_channel_datasets {
            let mut channels = Vec::<Hdf5Channel>::new();

            let channel_datasets = group.datasets()
                .expect("Datasets should be accessible, this should never fail.")
                .into_iter()
                .filter_map(|dataset|
                    (!["frame_number", "period_number", "timestamp"]
                        .contains(&dataset.name().split('/').last()?))
                        .then_some(dataset)
                );

            let channels = channel_datasets.into_iter().map(|dataset| {
                let channel: Channel = extract_from_dataset_name(dataset.name(), CHANNEL)?;
                assert_eq!(*dataset.shape(), vec![num_frames]);
                let channel = Hdf5Channel::new(channel,CachedDataset::new(dataset, config.cache_size.as_ref())?);
                Ok(channel)
            }).collect::<Result<Vec<_>,Error>>()?;
            Channels::MULTIPLE(channels)
        } else {
            let channels = group
                .dataset("channels")?
                .read_1d()?;
            let traces = group.dataset("traces")?;
            info!("Digitiser {digitiser_id} has traces dataset of size {:?}.", traces.shape());
            Channels::SINGLE(Hdf5AllChannels::new(channels, traces))
        };
        Ok(Hdf5Digitiser {
            digitiser_id,
            period_numbers,
            frame_numbers,
            timestamps,
            channels,
            num_frames
        })
    }

    pub(crate) fn get_index_from_frame_number(&self, frame_number: FrameNumber) -> Option<usize> {
        self.frame_numbers.iter()
            .enumerate()
            .find_map(|(i,v)|
                (frame_number.eq(v))
                    .then_some(i)
            )
    }

    #[tracing::instrument(skip_all)]
    pub(crate) fn ensure_elements_cached(&mut self, index: usize) {
        if let Timestamps::RFC3999(timestamps) = &mut self.timestamps {
            timestamps.ensure_elements_cached(index);
        }
        match &mut self.channels {
            Channels::MULTIPLE(hdf5_channels) => hdf5_channels.iter_mut()
                .for_each(|channel: &mut Hdf5Channel|
                    channel.ensure_elements_cached(index)
                ),
            Channels::SINGLE(hdf5_channel) => {
                hdf5_channel.ensure_elements_cached(index);
            }
        }
    }

    pub(crate) fn output_summary (&mut self) {
        println!("Digitiser: {}. Num Frames: {}", self.digitiser_id, self.frame_numbers.len());
        let frame_numbers = (0..self.frame_numbers.len())
            .map(|i| self.frame_numbers
                    .get(i)
                    .expect("Index should be in range, this should never fail.")
                );
        let output = match &mut self.timestamps {
            Timestamps::RFC3999(timestamps) => {
                let timestamps = (0..timestamps.get_num_elements())
                    .map(|i| {
                        timestamps.ensure_elements_cached(i);
                        let temp = timestamps.get_element(i)
                            .split(['T', '+'])
                            .skip(1).take(1)
                            .collect::<Vec<_>>();
                        temp[0].to_string()
                    });
                frame_numbers.zip(timestamps)
                    .map(|(f,t)|format!("{f}: {t}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
            Timestamps::EPOCHNS(timestamps) => {
                let timestamps = (0..timestamps.len())
                    .map(|i| {
                        DateTime::from_timestamp_nanos(*timestamps.get(i).unwrap()).to_rfc3339()
                    });
                frame_numbers.zip(timestamps)
                    .map(|(f,t)|format!("{f}: {t}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };
        
        println!("{output}");
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
    #[tracing::instrument(skip_all)]
    pub(crate) fn create_message(
        &self,
        fbb: &mut FlatBufferBuilder<'_>,
        index: usize,
        sampling_rate: u64,
        shift_timestamp_date_to_today: bool,
    ) -> Result<(), Error> {
        if index >= self.num_frames {
            warn!("Index {index} >= size {}", self.num_frames);
            return Ok(());
        }
        fbb.reset();

        let frame_number = *self.frame_numbers.get(index).expect("This should never fail.");
        let period_number = *self.period_numbers.get(index).expect("This should never fail.");
        let mut timestamp: DateTime<Utc> = match &self.timestamps {
            Timestamps::RFC3999(timestamps) => timestamps.get_element(index).parse()?,
            Timestamps::EPOCHNS(timestamps) => DateTime::from_timestamp_nanos(*timestamps.get(index).expect("This should never fail.")),
        };
        if shift_timestamp_date_to_today {
            timestamp = timestamp
                .with_day(Utc::now().day())
                .expect("This should never fail.")
                .with_year(Utc::now().year())
                .expect("This should never fail.");
        }

        let channels = match &self.channels {
            Channels::MULTIPLE(hdf5_channels) => {
                let trace = hdf5_channels
                    .iter()
                    .map(|c| c.create_channel(fbb, index))
                    .collect::<Vec<_>>();
                fbb.create_vector_from_iter(trace.iter())
            },
            Channels::SINGLE(hdf5_channel) => {
                hdf5_channel.create_channels(fbb, index)
            }
        };

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
            channels: Some(channels),
        };
        let message = DigitizerAnalogTraceMessage::create(fbb, &message);
        finish_digitizer_analog_trace_message_buffer(fbb, message);
        Ok(())
    }
}
