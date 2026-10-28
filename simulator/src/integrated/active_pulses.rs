use crate::integrated::simulation_elements::pulses::PulseEvent;
use digital_muon_common::Time;
use std::{collections::VecDeque, slice::Iter};

/// Represents a temporal window into a collection of [PulseEvent] elements, whose `start` and `end` fields
/// contain the current time value. Elements are added to the subset as the `time` value exceeds
/// their `start` field, and are dropped when the `time` passes their `end` field.
///
/// The underlying collection are assumed to be increasing in time.
pub(super) struct ActivePulses<'a> {
    /// Contiguous subset of the elements of [Self::muon_iter].
    active: VecDeque<&'a PulseEvent>,
    /// Iterates through a collection of
    muon_iter: Iter<'a, PulseEvent>,
}

impl<'a> ActivePulses<'a> {
    /// Creates a new instance with a given slice as the underlying collection.
    ///
    /// # Parameters
    /// - source: slice of the underlying collection.
    pub(super) fn new(source: &'a [PulseEvent]) -> Self {
        Self {
            active: Default::default(),
            muon_iter: source.iter(),
        }
    }

    /// Expunges any muons whose `end` field is less than the provided `time`.
    ///
    /// # Parameters
    /// - time: the current time of the simulation.
    pub(super) fn drop_spent_muons(&mut self, time: Time) {
        while self
            .active
            .front()
            .and_then(|m| (m.get_end() < time).then_some(m))
            .is_some()
        {
            self.active.pop_front();
        }
    }

    /// Adds any new muons whose `start` field is greater than the provided `time`.
    ///
    /// # Parameters
    /// - time: the current time of the simulation.
    pub(super) fn push_new_muons(&mut self, time: Time) {
        while let Some(iter) = self
            .muon_iter
            .next()
            .and_then(|iter| (iter.get_start() > time).then_some(iter))
        {
            self.active.push_back(iter)
        }
    }

    /// Iterator into the window.
    pub(super) fn iter(&self) -> std::collections::vec_deque::Iter<'_, &PulseEvent> {
        self.active.iter()
    }
}
