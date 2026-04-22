use digital_muon_common::Time;
use tracing::warn;
use crate::pulse_detection::Real;

/// Cache containing the time values.
/// 
/// In normal operation, these are written to only once,
/// however as the size of the trace and the sample time
/// are reported on a frame by frame basis, the size and
/// value of the sample time is checked each trace to permit
/// continued operation in the case these values change.
#[derive(Default, Clone)]
pub(crate) struct TimeCache {
    /// Value of `sample_time`
    expected_sample_time: Option<Real>,
    /// Memory in which to write the time bin values.
    time: Vec<Time>,
}

impl TimeCache {
    /// Refreshes the `time` vector if and only if the size of the vector changes, or the `sample_time` field.
    /// 
    /// If a change is detected (besides the initial setup), then a warning is issued.
    /// # Parameters
    /// - size: the intended size of the `time` vector.
    /// - sample_time: the intended `sample_time`, defining the scale of the time-series.
    pub(crate) fn ensure_time_data_written(&mut self, size: usize, sample_time: Real) {
        if size != self.time.len()
            || self
                .expected_sample_time
                .is_some_and(|current_sample_time| current_sample_time != sample_time)
        {
            if let Some(old_sample_time) = self.expected_sample_time.as_ref() {
                warn!("Change of sample time detected, from {old_sample_time} to {sample_time}.");
            }
            if self.time.len() != 0 {
                warn!("Change of trace length detected, from {} to {size}.", self.time.len());
            }
            self.time = (0..size).map(|t| (t as Real * sample_time) as Time).collect();
            self.expected_sample_time = Some(sample_time);
        }
    }

    /// Converts a list of trace indices into the corresponding time values that are stored in the cache.
    pub(crate) fn into_times(&self, indices: Vec<usize>) -> Vec<Time> {
        indices.into_iter().map(|index|*self.time.get(index).expect("Element should exist, this should never fail")).collect()
    }
}
