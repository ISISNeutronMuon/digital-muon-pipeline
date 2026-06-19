mod cached_dataset;
mod channel;
mod digitiser;


use chrono::ParseError;
use digital_muon_common::spanned::{Spanned, SpanWrapper};
use digital_muon_streaming_types::flatbuffers::FlatBufferBuilder;
use hdf5::{File, OpenMode};
use rayon::iter::{ParallelIterator, IntoParallelRefMutIterator};
use rdkafka::{
    ClientConfig, producer::{BaseRecord, DefaultProducerContext, ThreadedProducer}
};
use thiserror::Error;
use std::{num::ParseIntError, path::PathBuf};
use tracing::{debug, info_span};


use std::{fmt::Debug, str::FromStr};

pub(crate) use digitiser::{Hdf5Digitiser, HDF5Config};

use crate::Hdf5;

#[derive(Error, Debug)]
pub(crate) enum Error {
    #[error("Dataset {0} is a scalar, vector expected.")]
    DatasetScalar(String),
    #[error("Dataset name is zero-length")]
    DatasetNameZeroLength,
    #[error("Expecting Underscore in {0}")]
    NoUnderscore(String),
    #[error("Wrong Identifier. Expected {0}, got {1}")]
    WrongIdentifier(String,String),
    #[error("{0}")]
    ParseInt(#[from] ParseIntError),
    #[error("{0}")]
    HDF5(#[from] hdf5::Error),
    #[error("{0}")]
    DateTime(#[from] ParseError)
}

fn extract_from_dataset_name<'a, T>(name: String, identifier: &'static str) -> Result<T,Error>
where
    T: FromStr,
    <T as FromStr>::Err: Debug,
    Error: From<<T as FromStr>::Err>
{
    let group_name = name
        .split('/')
        .last()
        .ok_or(Error::DatasetNameZeroLength)?
        .split('_')
        .collect::<Vec<_>>();
    if group_name.len() < 1 {
        Err(Error::NoUnderscore(name.clone()))?;
    }
    if group_name.get(0).unwrap() != &identifier {
        Err(Error::WrongIdentifier(identifier.to_string(), group_name.first().expect("This should never fail.").to_string()))?
    }
    Ok(group_name.get(1)
        .expect("This should never fail.")
        .parse()?
    )
}

pub(crate) async fn read_hdf5_file(
    file_name: PathBuf,
    client_config: &ClientConfig,
    trace_topic: &str,
    args: Hdf5,
) {
    let file = {
        let mut file_builder = File::with_options();
        file_builder.fapl().stdio();//log_options(Some("mylog"), LogFlags::union(LogFlags::LOC_IO, LogFlags::TIME_READ), 0);//
        file_builder.open_as(file_name, OpenMode::Read).unwrap()
    };


    let config = HDF5Config {
        timestamp_as_rfc3339: file.attr("config_timestamp_as_rfc3339")
            .and_then(|config|config.read_scalar::<bool>())
            .unwrap_or(true),
        multiple_channel_datasets: file.attr("config_multiple_channel_datasets")
            .and_then(|config|config.read_scalar::<bool>())
            .unwrap_or(true),
        cache_size: args.cache_size,
        
    };
    debug!("File config: {config:?}");
    
    let mut digitisers = Hdf5Digitiser::open_from(file, config).unwrap()
        .into_iter()
        .map(|digitiser|DigitiserReader::new(client_config, &args, digitiser))
        .collect::<Vec<_>>();

    if args.summary_only {
        for digitiser in digitisers.iter_mut() {
            digitiser.digitiser.output_summary();
        }
    } else {
        let num_indices = digitisers.iter().map(|digitiser| digitiser.to_index - digitiser.from_index).min().unwrap();
        for index in 0..=num_indices {
            read_hdf5_at_index(&mut digitisers, trace_topic, &args, index).await;
        }
    }
}



struct DigitiserReader {
    digitiser: Hdf5Digitiser,
    from_index: usize,
    to_index: usize,
    producer: ThreadedProducer<DefaultProducerContext>
}

impl DigitiserReader {
    fn new(client_config: &ClientConfig, args: &Hdf5, digitiser: Hdf5Digitiser) -> Self {
        let from_index = args.from_index
            .unwrap_or(args.from_frame_number
                .and_then(|frame_number|digitiser.get_index_from_frame_number(frame_number))
                .unwrap_or_default()
            );
        let to_index = args.to_index
            .unwrap_or(args.to_frame_number
                .and_then(|frame_number|digitiser.get_index_from_frame_number(frame_number))
                .unwrap_or(digitiser.get_num_frames() - 1)
            );
        let producer = client_config.create().unwrap();
        Self {
            digitiser,
            from_index,
            to_index,
            producer
        }
    }

    fn read_at_index(&self, trace_topic: &str, args: &Hdf5, index: usize) {
            let mut fbb = FlatBufferBuilder::new();
            self.digitiser
                .create_message(&mut fbb, index, 1000000000, args.shift_to_today)
                .unwrap();
            info_span!("Send").in_scope(|| {
                let base_record = BaseRecord::to(trace_topic)
                    .payload(fbb.finished_data())
                    .key("");
                
                let mut result = self.producer.send(base_record);
                while let Err((_, base_record)) = result {
                    result = self.producer.send(base_record);
                }
            });
    }
}


#[tracing::instrument(skip_all)]
async fn read_hdf5_at_index(
    digitisers: &mut [DigitiserReader],
    trace_topic: &str,
    args: &Hdf5,
    index: usize,
) {
    let mut spanned_digitisers = digitisers
        .iter_mut()
        .map(|digitiser|SpanWrapper::<_>::new(info_span!("Digitiser"), digitiser))
        .collect::<Vec<_>>();

    spanned_digitisers.iter_mut()
        .for_each(|spanned_digitiser| {
            let index_on_digitiser = index + spanned_digitiser.from_index;
            let span = spanned_digitiser
                .span()
                .get()
                .expect("Digitiser has span")
                .clone();
            span.in_scope(||
                spanned_digitiser.digitiser.ensure_elements_cached(index_on_digitiser)
            );
        }
    );

    spanned_digitisers
        .par_iter_mut()
        .for_each(|spanned_digitiser| {
            let span = spanned_digitiser
                .span()
                .get()
                .expect("Digitiser has span");
            span.in_scope(|| spanned_digitiser.read_at_index(trace_topic, args, index))
    });
}


#[cfg(test)]
mod tests {
    use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::root_as_digitizer_analog_trace_message;
    use std::{fs::File, io::Read};
    use super::*;

    #[test]
    fn test() {
        let file = hdf5::File::open_as(PathBuf::from_str("test_assets/test.hdf5").unwrap(), OpenMode::Read).unwrap();
        let config = HDF5Config {
            timestamp_as_rfc3339: file.attr("config_timestamp_as_rfc3339")
                .and_then(|config|config.read_scalar::<bool>())
                .unwrap_or(true),
            multiple_channel_datasets: file.attr("config_multiple_channel_datasets")
                .and_then(|config|config.read_scalar::<bool>())
                .unwrap_or(true),
            cache_size: None,  
        };
        
        let digitisers = Hdf5Digitiser::open_from(file, config).unwrap();
        assert_eq!(digitisers.len(), 1);

        let mut fbb = FlatBufferBuilder::new();
        assert!(digitisers[0].create_message(&mut fbb, 0, 1_000_000_000, false).is_ok());
        let dat_test = root_as_digitizer_analog_trace_message(fbb.unfinished_data()).unwrap();

        let data = {
            let mut file = File::open("test_assets/test.dat2").unwrap();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            data
        };

        let dat_true = root_as_digitizer_analog_trace_message(&data).unwrap();
        assert_eq!(dat_test.digitizer_id(), dat_true.digitizer_id());
        assert_eq!(dat_test.metadata().frame_number(), dat_true.metadata().frame_number());
        assert_eq!(dat_test.metadata().period_number(), dat_true.metadata().period_number());
        assert_eq!(dat_test.metadata().protons_per_pulse(), dat_true.metadata().protons_per_pulse());
        assert_eq!(dat_test.metadata().running(), dat_true.metadata().running());
        assert_eq!(dat_test.metadata().timestamp(), dat_true.metadata().timestamp());
        for (channel_test, channel_true) in dat_test.channels().unwrap().iter().zip(dat_true.channels().unwrap().iter()) {
            assert_eq!(channel_test.channel(), channel_true.channel());
            assert!(channel_test.voltage().is_some());
            assert_eq!(channel_test.voltage().unwrap().iter().collect::<Vec<_>>(), channel_true.voltage().unwrap().iter().collect::<Vec<_>>());
        }
    }
}
