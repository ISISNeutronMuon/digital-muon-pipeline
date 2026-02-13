use crate::integrated::{
    build_messages::BuildError,
    simulation_elements::{
        DigitiserConfig, Transformation,
        event_list::{EventList, EventListTemplate, Trace},
        pulses::PulseTemplate,
        utils::{JsonValueError, NumConstant},
    },
    simulation_engine::actions::Action,
};
use chrono::Utc;
use digital_muon_common::{
    FrameNumber, Time,
    spanned::{SpanWrapper, Spanned},
};
use rand::SeedableRng;
use rand::distr::weighted::WeightedIndex;
use rand_distr::Distribution;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::Deserialize;
use thiserror::Error;
use tracing::instrument;

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Simulation {
    // Is applied to all voltages when traces are created
    pub(crate) voltage_transformation: Transformation<f64>,
    //  The length of each trace
    pub(crate) time_bins: NumConstant<Time>,
    //  Number of samples (time_bins) per second
    pub(crate) sample_rate: NumConstant<u64>,
    pub(crate) digitiser_config: DigitiserConfig,
    pub(crate) event_lists: Vec<EventListTemplate>,
    pub(crate) pulses: Vec<PulseTemplate>,
    pub(crate) schedule: Vec<Action>,
}

#[derive(Debug, Error)]
pub(crate) enum SimulationError {
    #[error("Event Pulse Template index {0} out of range {1}")]
    EventListIndexOutOfRange(usize, usize),
    #[error("Event Pulse Template index {0} out of range {1}")]
    EventPulseTemplateIndexOutOfRange(usize, usize),
    #[error("Json Float error: {0}")]
    JsonValue(#[from] JsonValueError),
    #[error("Build error: {0}")]
    Build(#[from] BuildError),
}

impl Simulation {
    #[instrument(skip_all, level = "debug", err(level = "error"))]
    pub(crate) fn get_random_pulse_template(
        &self,
        source: &EventListTemplate,
        distr: &WeightedIndex<f64>,
    ) -> Result<&PulseTemplate, SimulationError> {
        //  get a random index for the pulse
        let index = distr.sample(&mut rand::rngs::StdRng::seed_from_u64(
            Utc::now().timestamp_subsec_nanos() as u64,
        ));
        let event_pulse_template =
            source
                .pulses
                .get(index)
                .ok_or(SimulationError::EventPulseTemplateIndexOutOfRange(
                    index,
                    source.pulses.len(),
                ))?;
        // Return a pointer to either a local or global pulse
        self.pulses.get(event_pulse_template.pulse_index).ok_or(
            SimulationError::EventPulseTemplateIndexOutOfRange(index, source.pulses.len()),
        )
    }

    #[instrument(skip_all, err(level = "error"))]
    pub(crate) fn generate_event_lists(
        &self,
        index: usize,
        frame_number: FrameNumber,
        repeat: usize,
    ) -> Result<Vec<EventList<'_>>, SimulationError> {
        let source =
            self.event_lists
                .get(index)
                .ok_or(SimulationError::EventListIndexOutOfRange(
                    index,
                    self.event_lists.len(),
                ))?;

        let vec = (0..repeat)
            .map(SpanWrapper::<usize>::new_with_current)
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|span_wrapper| {
                span_wrapper
                    .span()
                    .get()
                    .expect("Span should exist, this never fails")
                    .in_scope(|| EventList::new(self, frame_number, source))
            })
            .collect::<Vec<Result<_, SimulationError>>>()
            .into_iter()
            .collect::<Result<_, _>>()?;
        Ok(vec)
    }

    #[instrument(skip_all, level = "debug", err(level = "error"))]
    pub(crate) fn generate_traces<'a>(
        &'a self,
        event_lists: &'a [EventList],
        frame_number: FrameNumber,
    ) -> Result<Vec<Trace>, JsonValueError> {
        event_lists
            .iter()
            .map(SpanWrapper::<_>::new_with_current)
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|event_list| {
                let current_span = event_list
                    .span()
                    .get()
                    .expect("Span should exist, this never fails"); //  This is the span of this method
                let event_list: &EventList = *event_list; //  This is the spanned event list
                current_span.in_scope(|| Trace::new(self, frame_number, event_list))
            })
            .collect::<Vec<Result<_, JsonValueError>>>()
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON_INPUT_1: &str = r#"
    {
        "voltage-transformation": {"scale": 1, "translate": 0 },
        "time-bins": { "const": 30000 },
        "sample-rate": { "const": 1000000000 },
        "digitiser-config": {
            "auto-digitisers": {
                "num-digitisers": { "const" : 32 },
                "num-channels-per-digitiser": { "const" : 8 }
            }
        },
        "pulses": [{
                        "pulse-type": "back-to-back-exp",
                        "spread":      { "random-type": "constant-float", "value": { "const": 5.5 } },
                        "rising":      { "random-type": "constant-float", "value": { "const": 3.5 } },
                        "falling":     { "random-type": "constant-float", "value": { "const": 2.4 } },
                        "peak_time":   { "random-type": "exponential", "lifetime": { "const": 2200 } },
                        "peak_height": { "random-type": "uniform-float", "min": { "const": 250 }, "max": { "const": 1100 } }
                    },
                    {
                        "pulse-type": "flat",
                        "start":  { "random-type": "exponential", "lifetime": { "const": 2200 } },
                        "width":  { "random-type": "uniform-float", "min": { "const": 20 }, "max": { "const": 50 } },
                        "height": { "random-type": "uniform-float", "min": { "const": 30 }, "max": { "const": 70 } }
                    },
                    {
                        "pulse-type": "triangular",
                        "start":     { "random-type": "exponential", "lifetime": { "const": 2200 } },
                        "width":     { "random-type": "uniform-float", "min": { "const": 20 }, "max": { "const": 50 } },
                        "peak_time": { "random-type": "uniform-float", "min": { "const": 0.25 }, "max": { "const": 0.75 } },
                        "height":    { "random-type": "uniform-float", "min": { "const": 30 }, "max": { "const": 70 } }
                    }],
        "event-lists": [
            {
                "pulses": [
                    {"weight": 1, "pulse-index": 0},
                    {"weight": 1, "pulse-index": 1},
                    {"weight": 1, "pulse-index": 2}
                ],
                "noises": [
                    {
                        "attributes": { "noise-type" : "gaussian", "mean" : { "const": 0 }, "sd" : { "const": 20 } },
                        "smoothing-factor" : { "const": 0.975 },
                        "bounds" : { "min": 0, "max": 30000 }
                    },
                    {
                        "attributes": { "noise-type" : "gaussian", "mean" : { "const": 0 }, "sd" : { "num-func": { "scale": 50, "translate": 50 } } },
                        "smoothing-factor" : { "const": 0.995 },
                        "bounds" : { "min": 0, "max": 30000 }
                    }
                ],
                "num-pulses": { "random-type": "constant-int", "value": { "const": 500 } }
            }
        ],
        "schedule": [
            { "send-run-start": { "name": { "text": "MyRun" }, "filename": { "text": "RunFile" }, "instrument": { "text": "MuSR" } } },
            { "set-timestamp": "now" },
            { "wait-ms": 100 },
            { "frame-loop": {
                    "start": { "const": 0 },
                    "end": { "const": 99 },
                    "schedule": [
                        { "set-timestamp": { "advance-by-ms" : 5} },
                        { "set-timestamp": { "rewind-by-ms" : 5} }
                    ]
                }
            }
        ]
    }
    "#;
    #[test]
    fn test1() {
        let simulation: Simulation = serde_json::from_str(JSON_INPUT_1).unwrap();

        assert_eq!(simulation.pulses.len(), 3);
        assert_eq!(simulation.voltage_transformation.scale, 1.0);
        assert_eq!(simulation.voltage_transformation.translate, 0.0);
    }
}
