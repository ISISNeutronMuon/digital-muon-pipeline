//! Defines the event list type, used for both digitiser messages and frame messages.
use digital_muon_common::{Channel, Intensity, Time};
use digital_muon_streaming_types::dev2_digitizer_event_v2_generated::DigitizerEventListMessage;
use std::collections::HashMap;

/// Event list, either for a digitiser message, or frame message.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChannelData {
    /// Time at which event occurred and its intensity value. Time is relative to frame metadata timestamp (ns).
    time_intensity: Vec<(Time, Intensity)>,
}

impl ChannelData {
    #[cfg(test)]
    pub(crate) fn new(time_intensity: Vec<(Time, Intensity)>) -> Self {
        Self { time_intensity }
    }

    /// Get the underlying data.
    pub(crate) fn get_time_intensity(&self) -> &[(Time, Intensity)] {
        &self.time_intensity
    }

    /// For a given index in the data, get the time distance from a given target time.
    pub(crate) fn get_temporal_distance_from(&self, index: usize, target: Time) -> u32 {
        let (time, _) = self
            .time_intensity
            .get(index)
            .expect("`index` should be valid, this should never fail");
        (*time as i32 - target as i32)
            .abs()
            .try_into()
            .expect("`abs()` should be positive, this should never fail.")
    }

    /// Selects the index of the datapoint nearest to the target between the given index, and its immediate successor.
    pub(crate) fn find_nearest_in_time_after_index(&self, index: usize, target: Time) -> usize {
        if self.get_temporal_distance_from(index, target)
            < self.get_temporal_distance_from(index + 1, target)
        {
            index
        } else {
            index + 1
        }
    }
}

/// Event list, either for a digitiser message, or frame message.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct EventData {
    /// Id of the detector which registered the event.
    pub(crate) events: HashMap<Channel, ChannelData>,
}

impl EventData {
    pub(crate) fn get_channels(&self) -> Vec<Channel> {
        self.events.keys().copied().collect()
    }
}

impl<'a> From<DigitizerEventListMessage<'a>> for EventData {
    fn from(msg: DigitizerEventListMessage<'a>) -> Self {
        let time = msg.time().expect("data should have times").iter();
        let intensity = msg.voltage().expect("data should have intensities").iter();
        let channel = msg
            .channel()
            .expect("data should have channel numbers")
            .iter();
        let mut events = HashMap::<Channel, ChannelData>::new();
        for (c, (t, i)) in channel.zip(Iterator::zip(time, intensity)) {
            let data = events.entry(c).or_default();
            data.time_intensity.push((t, i));
        }

        // The guarantee that all fields are of equal length depends on the inputs
        // having fields of equal length. This is guaranteed by the `trace-to-events`
        // unit so is not checked.
        Self { events }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let data = ChannelData {
            time_intensity: vec![(33, 0), (38, 0), (51, 0)],
        };
        assert_eq!(data.get_temporal_distance_from(0, 42), 9);
        assert_eq!(data.get_temporal_distance_from(1, 42), 4);
        assert_eq!(data.get_temporal_distance_from(2, 42), 9);

        assert_eq!(data.find_nearest_in_time_after_index(0, 32), 0);
        assert_eq!(data.find_nearest_in_time_after_index(0, 34), 0);
        assert_eq!(data.find_nearest_in_time_after_index(0, 42), 1);
        assert_eq!(data.find_nearest_in_time_after_index(1, 42), 1);
        assert_eq!(data.find_nearest_in_time_after_index(0, 51), 1);
        assert_eq!(data.find_nearest_in_time_after_index(1, 51), 2);
    }
}
