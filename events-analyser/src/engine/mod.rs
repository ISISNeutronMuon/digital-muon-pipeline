mod algorithm;
mod bucket;
mod chart;
mod criteria;
mod metric;
mod utils;
mod values;

use serde::Deserialize;

use crate::engine::algorithm::AlgorithmTemplate;
use crate::engine::bucket::BucketBlock;
use crate::engine::chart::Chart;
use crate::engine::criteria::Criteria;
use crate::engine::metric::Metric;

pub(crate) use crate::engine::bucket::FlatBucketBlock;

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Templates {
    // List of metrics to calculate for each phase.
    pub(crate) criteria_templates: Vec<Criteria>,
    // List of metrics to calculate for each phase.
    pub(crate) arrays: Vec<Array>,
    // List of metrics to calculate for each phase.
    pub(crate) algorithms: Vec<AlgorithmTemplate>,
}

impl Templates {
    fn get_criteria(&self) -> &[Criteria] {
        &self.criteria_templates
    }

    fn get_arrays(&self) -> &[Array] {
        &self.arrays
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct AnalysisSettings {
    // These topics are interpreted as the results of detector
    pub(crate) events_topics: Vec<String>,
    // List of metrics to calculate for each phase.
    pub(crate) metrics: Vec<Metric>,
    // List of metrics to calculate for each phase.
    pub(crate) templates: Templates,
    // List of phases in the analysis.
    pub(crate) buckets: Vec<BucketBlock>,
    // List of phases in the analysis.
    pub(crate) charts: Vec<Chart>,
}

trait Flattenable {
    type Flat;
    type Library: ?Sized;
    type Error;

    fn flatten(&self, library: &Self::Library) -> Result<Self::Flat, Self::Error>;
}

trait FlattenableWithIndex {
    type Flat;
    type Library: ?Sized;
    type Error;

    fn flatten(&self, library: &Self::Library, index: usize) -> Result<Self::Flat, Self::Error>;
}

impl AnalysisSettings {
    pub(crate) fn flatten_buckets(&self) -> Result<Vec<FlatBucketBlock>, String> {
        self.buckets
            .iter()
            .map(|block| block.flatten(&self.templates))
            .collect::<Result<_, String>>()
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct Array {
    // Is applied to all voltages when traces are created
    pub(crate) name: String,
    // Is applied to all voltages when traces are created
    pub(crate) values: Vec<f64>,
}

impl Array {
    pub(crate) fn has_name(&self, name: &str) -> bool {
        self.name == name
    }

    pub(crate) fn get_element(&self, index: usize) -> f64 {
        self.values.get(index).unwrap().clone() // FIXME: Handle Error
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
                        "smoothing-window-length" : { "const": 1 },
                        "bounds" : { "min": { "const": 0 }, "max": { "const": 30000 } }
                    },
                    {
                        "attributes": { "noise-type" : "gaussian", "mean" : { "const": 0 }, "sd" : { "num-func": { "scale": 50, "translate": 50 } } },
                        "smoothing-window-length" : { "const": 5 },
                        "bounds" : { "min": { "const": 0 }, "max": { "const": 30000 } }
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
        let simulation: AnalysisSettings = serde_json::from_str(JSON_INPUT_1).unwrap();
    }
}
