use chrono::{DateTime, Utc};
use digital_muon_common::FrameNumber;

/// Contains two metadata values of the same type, to represent
/// the first and latest value of the messages received.
///
/// In usage, `T` is intended to be either the timestamp or frame number.
pub(super) struct FirstAndLatest<T> {
    /// First metadata value received by the trace writer.
    first: T,
    /// Last metadata value received by the trace writer.
    latest: T,
}

impl<T: Copy> FirstAndLatest<T> {
    /// Create new struct from initial metadata value.
    ///
    /// # Parameters
    /// - data: latest value.
    pub(super) fn new(data: T) -> Option<Self> {
        Some(FirstAndLatest {
            first: data,
            latest: data,
        })
    }

    /// Append latest value to the latest field.
    ///
    /// # Parameters
    /// - data: latest value.
    pub(super) fn update(&mut self, data: T) {
        self.latest = data;
    }
}

impl FirstAndLatest<DateTime<Utc>> {
    /// Get the difference in duration between the first and latest timestamp.
    ///
    /// Note there is currently no error checking for non-monotonic timestamps.
    pub(super) fn get_difference_ms(&self) -> i64 {
        (self.latest - self.first).num_milliseconds()
    }
}

impl FirstAndLatest<FrameNumber> {
    /// Get the difference in frame count between the first and latest message.
    ///
    /// Note there is currently no error checking for non-monotonic frame numbers.
    pub(super) fn get_frames_difference(&self) -> i32 {
        (self.latest - self.first) as i32
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeDelta, Utc};
    use digital_muon_common::FrameNumber;
    use digital_muon_streaming_types::{
        dat2_digitizer_analog_trace_v2_generated::{
            DigitizerAnalogTraceMessage, DigitizerAnalogTraceMessageArgs,
            finish_digitizer_analog_trace_message_buffer, root_as_digitizer_analog_trace_message,
        },
        flatbuffers::FlatBufferBuilder,
        frame_metadata_v2_generated::{FrameMetadataV2, FrameMetadataV2Args},
    };

    use crate::{file_writer::TraceFileWriter, handle_termination_conditions};

    fn create_message<'a>(
        fbb: &'a mut FlatBufferBuilder,
        time: DateTime<Utc>,
        frame_number: FrameNumber,
    ) -> DigitizerAnalogTraceMessage<'a> {
        fbb.reset();
        let time = time.into();
        let metadata = FrameMetadataV2Args {
            frame_number,
            timestamp: Some(&time),
            period_number: 0,
            protons_per_pulse: 0,
            running: true,
            veto_flags: 0,
        };
        let message = DigitizerAnalogTraceMessageArgs {
            digitizer_id: 0,
            metadata: Some(FrameMetadataV2::create(fbb, &metadata)),
            sample_rate: 1_000_000_000,
            channels: None,
        };
        let message = DigitizerAnalogTraceMessage::create(fbb, &message);
        finish_digitizer_analog_trace_message_buffer(fbb, message);
        root_as_digitizer_analog_trace_message(fbb.finished_data()).unwrap()
    }

    #[test]
    fn test_handle_termination_conditions() {
        let mut fbb = FlatBufferBuilder::new();
        let mut writer = TraceFileWriter::new_temp("Null").unwrap();

        assert!(!handle_termination_conditions(Some(5), None, Some(&writer)));

        let time = Utc::now();
        let message = create_message(&mut fbb, time, 0);
        writer.write_trace_message(&message).unwrap();
        // Should not terminate.
        assert!(!handle_termination_conditions(None, None, None));
        assert!(!handle_termination_conditions(Some(0), Some(1), None));
        assert!(!handle_termination_conditions(None, None, Some(&writer)));
        assert!(!handle_termination_conditions(Some(0), None, Some(&writer)));
        assert!(!handle_termination_conditions(
            Some(0),
            Some(0),
            Some(&writer)
        ));
        assert!(!handle_termination_conditions(
            Some(0),
            Some(1),
            Some(&writer)
        ));
        assert!(!handle_termination_conditions(Some(5), None, Some(&writer)));

        let time = time.checked_add_signed(TimeDelta::milliseconds(5)).unwrap();
        let message = create_message(&mut fbb, time, 0);
        writer.write_trace_message(&message).unwrap();
        // Should not terminate.
        assert!(!handle_termination_conditions(Some(5), None, Some(&writer)));
        assert!(!handle_termination_conditions(
            Some(5),
            Some(0),
            Some(&writer)
        ));
        assert!(!handle_termination_conditions(
            Some(5),
            Some(1),
            Some(&writer)
        ));
        // Should terminate.
        assert!(handle_termination_conditions(Some(4), None, Some(&writer)));

        let time = time.checked_add_signed(TimeDelta::milliseconds(1)).unwrap();
        let message = create_message(&mut fbb, time, 0);
        writer.write_trace_message(&message).unwrap();
        // Should terminate.
        assert!(handle_termination_conditions(Some(5), None, Some(&writer)));

        let message = create_message(&mut fbb, time, 1);
        writer.write_trace_message(&message).unwrap();
        // Should not terminate.
        assert!(!handle_termination_conditions(Some(6), None, Some(&writer)));
        assert!(!handle_termination_conditions(
            Some(6),
            Some(1),
            Some(&writer)
        ));
        assert!(!handle_termination_conditions(None, Some(1), Some(&writer)));
        assert!(!handle_termination_conditions(None, None, Some(&writer)));
        // Should terminate.
        assert!(handle_termination_conditions(None, Some(0), Some(&writer)));

        let message = create_message(&mut fbb, time, 2);
        writer.write_trace_message(&message).unwrap();
        // Should not terminate.
        assert!(!handle_termination_conditions(None, Some(2), Some(&writer)));
        // Should terminate.
        assert!(handle_termination_conditions(None, Some(0), Some(&writer)));
        assert!(handle_termination_conditions(None, Some(1), Some(&writer)));
    }
}
