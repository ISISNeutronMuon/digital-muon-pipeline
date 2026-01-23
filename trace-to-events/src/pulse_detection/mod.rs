//! Provides tools for converting raw trace data into
//! a stream of events which represent pulses in the trace stream.
//!
//! A raw trace takes the form of a Vec (or some other similar container)
//! of scalar values. Typical usage of this crate may look like:
//! ```rust
//! let events = trace.iter()
//!     .enumerate()
//!     .map(|(i, v)| (i as Real * sample_time, v as Real))        // converts to (Real,Real) format.
//!     .window(SmoothedWindow::new(4))                            // A moving average window of length 4.
//!     .events(ThresholdDetector::new(                            // Registers an event when the averaged
//!         ThresholdDurationWrapper::from_str("5,1,0")            // signal exceeds 5 for 1 sample, with
//!             .unwrap()                                          // a cool-down of 0 samples
//!         )
//!     )
//! ```

pub(crate) mod datatype;
pub(crate) mod pulse;

pub(crate) mod detectors;
pub(crate) mod iterators;
pub(crate) mod window;

pub(crate) use datatype::{EventData, EventPoint, RealArray, Stats, Temporal, TracePoint};
pub(crate) use detectors::{Assembler, Detector, advanced_muon_detector, threshold_detector};
pub(crate) use iterators::{AssembleIterable, EventsIterable, WindowIterable};

pub(crate) use pulse::Pulse;

/// Standard type to use for real numbers.
pub(crate) type Real = f64;
