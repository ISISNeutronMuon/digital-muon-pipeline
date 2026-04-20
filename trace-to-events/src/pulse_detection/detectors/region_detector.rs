//! This detector breaks the stream into regions whose second derivative is greater or equal to a given threshold.
use crate::pulse_detection::{Detector, EventData, Real};

impl EventData for usize {}

/// (start, end) pair defining a region.
pub(crate) type RegionEvent = (usize, usize);

/// Detects pulses in a trace by analysing the differential of the trace.
#[derive(Default, Clone)]
pub(crate) struct RegionDetector {
    /// The detection parameters, a region is detected whenever the trace goes below this value.
    threshold: Real,
    /// If specified, only detect regions of at least this size.
    min_size: Option<usize>,

    /// The current state of the detector.
    partial_region: Option<RegionEvent>,
}

impl RegionDetector {
    /// Create new detector.
    pub(crate) fn new(threshold: Real, min_size: Option<usize>) -> Self {
        Self {
            threshold,
            min_size,
            ..Default::default()
        }
    }

    /// If a partial region exists, then return it if and only if it is of sufficient size, otherwise return `None`.
    fn filter_partial_region(&mut self) -> Option<RegionEvent> {
        self.partial_region.take().and_then(|partial_region| {
            self.min_size
                .is_none_or(|min_size| partial_region.1 >= min_size + partial_region.0)
                .then_some(partial_region)
        })
    }
}

impl Detector for RegionDetector {
    type TracePointType = (usize, Real);
    type EventOutputType = RegionEvent;

    fn signal(&mut self, time: usize, value: Real) -> Option<Self::EventOutputType> {
        if value >= self.threshold {
            // If the second derivative is above the threshold value,
            // filter and return any partial region.
            self.filter_partial_region()
        } else {
            // Otherwise, set the current partial region's right-bound, to the current time
            // (inserting a new one if necessary), and return None.
            self.partial_region
                .get_or_insert_with(|| (time, Default::default()))
                .1 = time;
            None
        }
    }

    fn finish(&mut self) -> Option<Self::EventOutputType> {
        self.filter_partial_region()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pulse_detection::{
            EventsIterable, Real, detectors::local_arg_min_detector::LocalArgMinDetector,
            utils::std_dev,
        },
        test_data::smoothing,
    };

    #[test]
    fn detect_regions_no_minsize() {
        let noise_std = std_dev(
            &smoothing::SECOND_DERIV[((0.9 * smoothing::SECOND_DERIV.len() as Real) as usize)..],
        )
        .unwrap();
        let pulses = smoothing::SECOND_DERIV
            .iter()
            .enumerate()
            .map(|(i, v)| (i, *v))
            .events(RegionDetector::new(-noise_std * 5.0, None))
            .flat_map(|region| {
                smoothing::SECOND_DERIV
                    .iter()
                    .cloned()
                    .enumerate()
                    .take(region.1)
                    .skip(region.0)
                    .events(LocalArgMinDetector::default())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(pulses, vec![8, 38]);
    }

    #[test]
    fn detect_regions_minsize_two() {
        let noise_std = std_dev(
            &smoothing::SECOND_DERIV[((0.9 * smoothing::SECOND_DERIV.len() as Real) as usize)..],
        )
        .unwrap();
        let pulses = smoothing::SECOND_DERIV
            .iter()
            .enumerate()
            .map(|(i, v)| (i, *v))
            .events(RegionDetector::new(-noise_std * 5.0, Some(5)))
            .flat_map(|region| {
                smoothing::SECOND_DERIV
                    .iter()
                    .cloned()
                    .enumerate()
                    .take(region.1)
                    .skip(region.0)
                    .events(LocalArgMinDetector::default())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(pulses, vec![38]);
    }
}
