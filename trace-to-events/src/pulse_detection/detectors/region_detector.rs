//! This detector breaks the stream into regions whose second derivative is greater or equal to a given threshold.
use crate::pulse_detection::{Detector, EventData, EventPoint, Real};

/// Represents a region of the trace.
pub(crate) type Data = usize;

impl EventData for Data {}

impl EventPoint for Data {
    type TimeType = usize;
    type EventType = Self;
}

/// (Time, Data) pair defining a pulse detection event.
pub(crate) type RegionEvent = (usize, Data);

/// Detects pulses in a trace by analysing the differential of the trace.
#[derive(Default, Clone)]
pub(crate) struct RegionDetector {
    /// The detection parameters.
    threshold: Real,
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

    fn filter_partial_region(&mut self) -> Option<RegionEvent> {
        self.partial_region.take().and_then(|partial_region|
            self.min_size
                .is_none_or(|min_size| partial_region.1 >= min_size + partial_region.0)
                .then_some(partial_region))
/*        if self.partial_region.is_none() {
            // If there is no partial region, then do nothing and return None.
            None
        } else {
            // Otherwise, take ownership, filter by min_size (if set), and return as Some.
            let partial_region = take(&mut self.partial_region);
            self.min_size
                .is_none_or(|min_size| partial_region.len() >= min_size)
                .then_some(partial_region)
        } */
    }
}

impl Detector for RegionDetector {
    type TracePointType = (usize,Real);
    type EventPointType = RegionEvent;

    fn signal(&mut self, time: usize, value: Real) -> Option<RegionEvent> {
        if value > self.threshold {
            // If the second derivative is above the threshold value,
            // filter and return any partial region.
            self.filter_partial_region()
        } else {
            // Otherwise, set the current partial region's right-bound, to the current time
            // (inserting a new one if necessary), and return None.
            self.partial_region.get_or_insert_with(||(time,Default::default())).1 = time;
            None
        }
    }

    fn finish(&mut self) -> Option<Self::EventPointType> {
        self.filter_partial_region()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse_detection::{
        EventsIterable, Real, detectors::local_arg_min_detector::LocalArgMinDetector, utils::stddev
    };

    const NX: usize = 85;

    const SECOND_DERIV: [f64; NX] = [
        0.0019666316857841204,
        0.0029481654822507627,
        0.004217218973127851,
        0.0045804936548089065,
        0.0030627572719094456,
        -0.0001717535408413684,
        -0.0037061879002541626,
        -0.005904431456213291,
        -0.006101182658202364,
        -0.004810390664778397,
        -0.0030088690608821578,
        -0.0014089823987382122,
        -0.0002755861656102837,
        0.00038696373449405297,
        0.0006613194418802759,
        0.0006992136845354324,
        0.0006999148378298431,
        0.0007591614817929332,
        0.0007706527106415839,
        0.000601268750823919,
        0.00034560005643863995,
        0.00026359645692117933,
        0.000444442017662236,
        0.0006576840046531793,
        0.0006022075321287096,
        0.00024251611746613916,
        -0.00014440210394809846,
        -0.0002008173577773449,
        0.0002559675308851303,
        0.001134812569280868,
        0.002148116287731424,
        0.002958206053970673,
        0.0032565834622328704,
        0.0027818259850227525,
        0.0014416546967605179,
        -0.000460448617937044,
        -0.0022460968964522,
        -0.0033283224370474207,
        -0.0036505643899789886,
        -0.003528171354201262,
        -0.0031338817685128,
        -0.0023607836046978803,
        -0.0011859001088510152,
        5.1586360512681506e-5,
        0.0008526772553082845,
        0.0009895691730946801,
        0.0006781444063808373,
        0.00034726147661325035,
        0.0002739895843861459,
        0.0004166359159440622,
        0.0005571827659502926,
        0.000572043609394942,
        0.000533253590558936,
        0.0005481068636608716,
        0.0005897458106075282,
        0.0005511199925585762,
        0.00040481398602523755,
        0.00022389106410612009,
        8.050242825633283e-5,
        -1.3210067212773152e-5,
        -6.989256252293644e-5,
        -7.797968368021546e-5,
        -2.0611120353796614e-5,
        5.412267427522613e-5,
        4.262400466534255e-5,
        -7.259552977117653e-5,
        -0.00014731080955054238,
        -4.1810668371399656e-5,
        0.00016642478832315857,
        0.0002457829186583403,
        9.928544540175124e-5,
        -9.410120975830716e-5,
        -9.000377397003212e-5,
        0.00011634205177022544,
        0.0002847738566986377,
        0.00021727938498912122,
        -4.2352053459020905e-5,
        -0.0003073736463266308,
        -0.00044273663906001237,
        -0.0004234314541211627,
        -0.00028523555516220614,
        -8.137402655841383e-5,
        0.00014867708124972207,
        0.00037478395984465694,
        0.0005291963545345068,
    ];

    #[test]
    fn detect_regions_no_minsize() {
        let noise_std = stddev(
            SECOND_DERIV
                .iter()
                .skip((0.9 * SECOND_DERIV.len() as Real) as usize)
                .cloned(),
        )
        .unwrap();
        let pulses = SECOND_DERIV
            .iter()
            .enumerate()
            .map(|(i, v)| (i, *v))
            .events(RegionDetector::new(-noise_std * 5.0, None))
            .flat_map(|region| {
                SECOND_DERIV.iter()
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
        let noise_std = stddev(
            SECOND_DERIV
                .iter()
                .skip((0.9 * SECOND_DERIV.len() as Real) as usize)
                .cloned(),
        )
        .unwrap();
        let pulses = SECOND_DERIV
            .iter()
            .enumerate()
            .map(|(i, v)| (i, *v))
            .events(RegionDetector::new(-noise_std * 5.0, Some(5)))
            .flat_map(|region| {
                SECOND_DERIV.iter()
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
