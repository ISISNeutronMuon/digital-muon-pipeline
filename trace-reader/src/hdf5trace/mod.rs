mod cached_dataset;
mod channel;
mod digitiser;

use crate::Hdf5;
use chrono::ParseError;
use digital_muon_common::{
    DigitizerId,
    spanned::{SpanWrapper, Spanned},
};
use digital_muon_streaming_types::flatbuffers::FlatBufferBuilder;
use hdf5::{File, OpenMode};
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use rdkafka::{
    ClientConfig,
    error::KafkaError,
    producer::{BaseRecord, DefaultProducerContext, ThreadedProducer},
};
use std::{fmt::Debug, num::ParseIntError, path::PathBuf, str::FromStr};
use thiserror::Error;
use tracing::{debug, info_span};

pub(crate) use digitiser::{HDF5Config, Hdf5Digitiser};

#[derive(Error, Debug)]
pub(crate) enum Error {
    #[error("Dataset name is zero-length")]
    DatasetNameZeroLength,
    #[error("Dataset {0} is a scalar, vector expected.")]
    DatasetScalar(String),
    #[error("{0}")]
    DateTime(#[from] ParseError),
    #[error("{0}")]
    HDF5(#[from] hdf5::Error),
    #[error("{0}")]
    Kafka(#[from] KafkaError),
    #[error("No digitisers from {0:?} selected.")]
    NoDigitisersSelected(Vec<DigitizerId>),
    #[error("Expecting Underscore in {0}")]
    NoUnderscore(String),
    #[error("{0}")]
    ParseInt(#[from] ParseIntError),
    #[error("Wrong Identifier. Expected {0}, got {1}")]
    WrongIdentifier(String, String),
}

/// Extracts the `index` from a string of the form `.../identifier_index`,
/// where `identifier` is the expected name, for instance "Digitiiser" or "Channel".
///
/// # Parameters
/// - source: the string to extract from.
/// - identifier: determines the expected format of the string.
///
/// # Returns
/// A value of type T, where T can be parsed from a string stlice.
///
/// # Errors
/// Returns an error if:
/// - `source` is zero length.
/// - `source` has no underscore.
/// - `source` has the wrong identifier.
fn extract_from_dataset_name<T>(source: String, identifier: &'static str) -> Result<T, Error>
where
    T: FromStr,
    <T as FromStr>::Err: Debug,
    Error: From<<T as FromStr>::Err>,
{
    let source_parts = source
        .split('/')
        .next_back()
        .ok_or(Error::DatasetNameZeroLength)?
        .split('_')
        .collect::<Vec<_>>();
    if source_parts.len() < 2 {
        Err(Error::NoUnderscore(source.clone()))?;
    }
    if source_parts.first().expect("First part should exist, this should never fail.") != &identifier {
        Err(Error::WrongIdentifier(
            identifier.to_string(),
            source_parts
                .first()
                .expect("First part should exist, this should never fail.")
                .to_string(),
        ))?
    }
    Ok(source_parts
        .get(1)
        .expect("Second part should exist, this should never fail.")
        .parse()?)
}

/// Runs the main loop when the program is run in `hdf5` mode.
///
/// # Parameters
/// - file_name: the file to read.
/// - client_config: the kafka config settings to use for the produer.
/// - trace_topic: the topic to produce trace messages to.
/// - args: the cli args specific to `hdf5` mode.
pub(crate) async fn read_hdf5_file(
    file_name: PathBuf,
    client_config: &ClientConfig,
    trace_topic: &str,
    key: &str,
    args: Hdf5,
) -> Result<(), Error> {
    // FIXME: Figure out which is the best file reader to use, probably `stdio` or `sec2`.
    // Also should we allow a logging option for debugging?
    let file = {
        let mut file_builder = File::with_options();
        file_builder.fapl().stdio();
        //file_builder.fapl().log_options(Some("mylog"), LogFlags::union(LogFlags::LOC_IO, LogFlags::TIME_READ), 0);
        file_builder.open_as(file_name, OpenMode::Read)?
    };

    let config = HDF5Config {
        timestamp_as_rfc3339: file
            .attr("config_timestamp_as_rfc3339")
            .and_then(|config| config.read_scalar::<bool>())
            .unwrap_or(true),
        multiple_channel_datasets: file
            .attr("config_multiple_channel_datasets")
            .and_then(|config| config.read_scalar::<bool>())
            .unwrap_or(true),
        cache_size: args.cache_size,
    };
    debug!("File config: {config:?}");

    let digitisers = Hdf5Digitiser::open_from(file, config)?
        .into_iter()
        .map(|digitiser| DigitiserReader::new(client_config, &args, digitiser))
        .collect::<Result<Vec<_>, Error>>()?;

    let digitiser_present = digitisers
        .iter()
        .map(|d| d.digitiser.get_id())
        .collect::<Vec<_>>();

    let mut digitisers = digitisers
        .into_iter()
        .filter(|d| d.is_id_contained_in(&args.digitizer_id))
        .collect::<Vec<_>>();

    if args.summary_only {
        for digitiser in digitisers.iter_mut() {
            digitiser.digitiser.output_summary();
        }
    } else {
        let num_indices = digitisers
            .iter()
            .map(|digitiser| digitiser.to_index - digitiser.from_index)
            .min()
            .ok_or(Error::NoDigitisersSelected(digitiser_present))?;
        for index in 0..=num_indices {
            read_hdf5_at_index(&mut digitisers, trace_topic, key, &args, index).await?;
        }
    }

    Ok(())
}

/// Encapsulates the tools needed to read the digitiser messages from a hdf5 file and produce them to the kafka broker.
struct DigitiserReader {
    /// Encapsulates the metadata and link to the hdf5 file for the digitiser messages.
    digitiser: Hdf5Digitiser,
    /// The index of the message to read from.
    from_index: usize,
    /// The index of the message to read to.
    to_index: usize,
    /// The kafka producer this digitiser uses.
    producer: ThreadedProducer<DefaultProducerContext>,
}

impl DigitiserReader {
    /// Creates a new instance.
    ///
    /// # Parameters
    /// - client_config: the kafka config settings to use for the produer.
    /// - args: the cli args specific to `hdf5` mode.
    /// - digitiser:
    fn new(
        client_config: &ClientConfig,
        args: &Hdf5,
        digitiser: Hdf5Digitiser,
    ) -> Result<Self, Error> {
        let from_index = args.from_index.unwrap_or(
            args.from_frame_number
                .and_then(|frame_number| digitiser.get_index_from_frame_number(frame_number))
                .unwrap_or_default(),
        );
        let to_index = args.to_index.unwrap_or(
            args.to_frame_number
                .and_then(|frame_number| digitiser.get_index_from_frame_number(frame_number))
                .unwrap_or(digitiser.get_num_frames() - 1),
        );
        let producer = client_config.create()?;
        Ok(Self {
            digitiser,
            from_index,
            to_index,
            producer,
        })
    }

    /// Read the digitiser message at the given index and produce it to the broker.
    ///
    /// # Parameters
    /// - trace_topic: the Kafka topic to produce to.
    /// - key: the text to use for the produced message's key.
    /// - args: the cli args specific to `hdf5` mode.
    /// - index: the index of the message to read.
    fn read_at_index(
        &self,
        trace_topic: &str,
        key: &str,
        args: &Hdf5,
        index: usize,
    ) -> Result<(), Error> {
        let mut fbb = FlatBufferBuilder::new();
        self.digitiser
            .create_message(&mut fbb, index, args.sample_rate, args.shift_to_today)?;
        info_span!("Send").in_scope(|| self.send_record(&mut fbb, trace_topic, key));
        Ok(())
    }

    /// Sends the FlatBuffer payload to the desired Kafka topic.
    ///
    /// # Parameters
    /// - fbb: mutable reference to the FlatBufferBuilder to use.
    /// - trace_topic: the Kafka topic to produce to.
    /// - key: the text to use for the produced message's key.
    fn send_record(&self, fbb: &mut FlatBufferBuilder, trace_topic: &str, key: &str) {
        let base_record = BaseRecord::to(trace_topic)
            .payload(fbb.finished_data())
            .key(key);

        let mut result = self.producer.send(base_record);
        while let Err((_, base_record)) = result {
            result = self.producer.send(base_record);
        }
    }

    fn is_id_contained_in(&self, ids: &[DigitizerId]) -> bool {
        if ids.is_empty() {
            true
        } else {
            ids.contains(&self.digitiser.get_id())
        }
    }
}

/// Read the messages at the given index, in the given slice of `DigitiserReaders` and produce them to the broker.
///
/// # Parameters
/// - digitisers: the slice of digitisers to operate on.
/// - trace_topic: the Kafka topic to produce to.
/// - key: the text to use for the produced message's key.
/// - args: the cli args specific to `hdf5` mode.
/// - index: the index of the message to read.
#[tracing::instrument(skip_all)]
async fn read_hdf5_at_index(
    digitisers: &mut [DigitiserReader],
    trace_topic: &str,
    key: &str,
    args: &Hdf5,
    index: usize,
) -> Result<(), Error> {
    let mut spanned_digitisers = digitisers
        .iter_mut()
        .map(|digitiser| SpanWrapper::<_>::new(info_span!("Digitiser"), digitiser))
        .collect::<Vec<_>>();

    spanned_digitisers.iter_mut().for_each(|spanned_digitiser| {
        let index_on_digitiser = index + spanned_digitiser.from_index;
        let span = spanned_digitiser
            .span()
            .get()
            .expect("Digitiser has span")
            .clone();
        span.in_scope(|| {
            spanned_digitiser
                .digitiser
                .ensure_elements_cached(index_on_digitiser)
        });
    });

    spanned_digitisers
        .par_iter_mut()
        .map(|spanned_digitiser| {
            let span = spanned_digitiser.span().get().expect("Digitiser has span");
            span.in_scope(|| spanned_digitiser.read_at_index(trace_topic, key, args, index))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use digital_muon_streaming_types::dat2_digitizer_analog_trace_v2_generated::root_as_digitizer_analog_trace_message;
    use std::{fs::File, io::Read};

    #[test]
    fn test() {
        let file = hdf5::File::open_as(
            PathBuf::from_str("test_assets/test.hdf5").unwrap(),
            OpenMode::Read,
        )
        .unwrap();
        let config = HDF5Config {
            timestamp_as_rfc3339: file
                .attr("config_timestamp_as_rfc3339")
                .and_then(|config| config.read_scalar::<bool>())
                .unwrap_or(true),
            multiple_channel_datasets: file
                .attr("config_multiple_channel_datasets")
                .and_then(|config| config.read_scalar::<bool>())
                .unwrap_or(true),
            cache_size: None,
        };

        let digitisers = Hdf5Digitiser::open_from(file, config).unwrap();
        assert_eq!(digitisers.len(), 1);

        let mut fbb = FlatBufferBuilder::new();
        assert!(
            digitisers[0]
                .create_message(&mut fbb, 0, 1_000_000_000, false)
                .is_ok()
        );
        let dat_test = root_as_digitizer_analog_trace_message(fbb.unfinished_data()).unwrap();

        let data = {
            let mut file = File::open("test_assets/test.dat2").unwrap();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            data
        };

        let dat_true = root_as_digitizer_analog_trace_message(&data).unwrap();
        assert_eq!(dat_test.digitizer_id(), dat_true.digitizer_id());
        assert_eq!(
            dat_test.metadata().frame_number(),
            dat_true.metadata().frame_number()
        );
        assert_eq!(
            dat_test.metadata().period_number(),
            dat_true.metadata().period_number()
        );
        assert_eq!(
            dat_test.metadata().protons_per_pulse(),
            dat_true.metadata().protons_per_pulse()
        );
        assert_eq!(dat_test.metadata().running(), dat_true.metadata().running());
        assert_eq!(
            dat_test.metadata().timestamp(),
            dat_true.metadata().timestamp()
        );
        for (channel_test, channel_true) in dat_test
            .channels()
            .unwrap()
            .iter()
            .zip(dat_true.channels().unwrap().iter())
        {
            assert_eq!(channel_test.channel(), channel_true.channel());
            assert!(channel_test.voltage().is_some());
            assert_eq!(
                channel_test.voltage().unwrap().iter().collect::<Vec<_>>(),
                channel_true.voltage().unwrap().iter().collect::<Vec<_>>()
            );
        }
    }
}
