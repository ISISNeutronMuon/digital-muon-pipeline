use std::{fmt::Debug, ops::Deref};
use crate::{
    engine::{
        Flattenable, FlattenableWithIndex, Templates,
        elements::{
            algorithm::FlatAlgorithm,
            criteria::{Criteria, CriteriaError, FlatCriteria},
            waveform::FlatWaveform,
        },
        utils::{Interval, HasName, HasSource},
        values::ValueError,
    },
    eventlists::EventlistsCollection,
};
use digital_muon_common::spanned::{SpanOnce, SpanOnceError, Spanned, SpannedAggregator, SpannedMut};
use serde::Deserialize;
use thiserror::Error;
use tracing::{info_span, instrument};

#[derive(Debug, Error)]
pub(crate) enum BucketError {
    #[error("Number not found in instance, or source {0}")]
    NoNumber(String),
    #[error("Algorithm not found in instance, or source {0}")]
    NoAlgorithm(String),
    #[error("Waveform not found in instance, or source {0}")]
    NoWaveform(String),
    #[error("Limits not found in instance, or source {0}")]
    NoLimits(String),
    #[error("Cannot find algorithm {0}.")]
    CannotFindAlgorithm(String),
    #[error("Cannot find waveform {0}.")]
    CannotFindWaveform(String),
    #[error("Value Error: {0}")]
    Value(#[from] ValueError),
    #[error("Value Error: {0}")]
    Criteria(#[from] CriteriaError),
    #[error("Span Error: {0}")]
    Span(#[from] SpanOnceError)
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct BucketBlockTemplate {
    pub(crate) name: String,
    #[serde(flatten)]
    pub(crate) properties: BucketBlockProperties,
}

impl HasName for BucketBlockTemplate {
    fn get_name(&self) -> &str {
        &self.name
    }
}

impl Deref for BucketBlockTemplate {
    type Target = BucketBlockProperties;

    fn deref(&self) -> &Self::Target {
        &self.properties
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct BucketBlockProperties {
    // Is applied to all voltages when traces are created
    pub(crate) number: Option<usize>,
    // Is applied to all voltages when traces are created
    pub(crate) algorithm: Option<String>,
    // Is applied to all voltages when traces are created
    pub(crate) waveform: Option<String>,
    // Is applied to all voltages when traces are created
    pub(crate) limits: Option<Interval<usize>>,
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct BucketBlock {
    pub(crate) source: String,
    pub(crate) name: String,
    // Is applied to all voltages when traces are created
    pub(crate) criteria: Criteria,
    // Is applied to all voltages when traces are created
    #[serde(flatten)]
    pub(crate) properties: BucketBlockProperties,
}

impl HasSource for BucketBlock {
    fn get_source(&self) -> &str {
        &self.source
    }
}

impl HasName for BucketBlock {
    fn get_name(&self) -> &str {
        &self.name
    }
}

impl Deref for BucketBlock {
    type Target = BucketBlockProperties;

    fn deref(&self) -> &Self::Target {
        &self.properties
    }
}

impl Flattenable<&Templates> for BucketBlock {
    type Flat = FlatBucketBlock;
    type Error = BucketError;

    #[instrument(skip_all, name = "Bucket Block")]
    fn flatten(&self, library: &Templates) -> Result<FlatBucketBlock, Self::Error> {
        let template = library.get_bucket(self);
        let number = self
            .properties
            .number
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.number.as_ref()))
            .ok_or_else(|| BucketError::NoNumber(self.get_source().into()))?;
        let algorithm = self
            .properties
            .algorithm
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.algorithm.as_ref()))
            .ok_or_else(|| BucketError::NoAlgorithm(self.get_source().into()))?;
        let waveform = self
            .properties
            .waveform
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.waveform.as_ref()))
            .ok_or_else(|| BucketError::NoWaveform(self.get_source().into()))?;
        let limits = self
            .properties
            .limits
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.limits.as_ref()))
            .ok_or_else(|| BucketError::NoLimits(self.get_source().into()))?;

        let algorithm = library
            .get_algorithm(&algorithm)
            .ok_or_else(|| BucketError::CannotFindAlgorithm(algorithm.into()))?;

        let waveform = library
            .get_waveform(&waveform)
            .ok_or_else(|| BucketError::CannotFindWaveform(waveform.into()))?;

        let buckets = (0..*number)
            .map(|index| {
                let criteria = self.criteria.flatten(library, index)?;
                let algorithm = algorithm.flatten(library.get_arrays(), index)?;
                let waveform = waveform.flatten(&library.arrays, index)?;
                let mut bucket = FlatBucket {
                    span: SpanOnce::default(),
                    criteria,
                    algorithm,
                    waveform,
                    count: Default::default(),
                };
                bucket.span_init()?;
                Ok(bucket)
            })
            .collect::<Result<Vec<_>, Self::Error>>()?;
        Ok(FlatBucketBlock {
            name: self.get_name().to_string(),
            span: SpanOnce::Spanned(tracing::Span::current()),
            buckets,
            limits: limits.clone(),
        })
    }
}

///
/// This struct is created from the configuration JSON file.
///
pub(crate) struct FlatBucketBlock {
    pub(crate) name: String,
    span: SpanOnce,
    // Is applied to all voltages when traces are created
    pub(crate) buckets: Vec<FlatBucket>,
    // Is applied to all voltages when traces are created
    pub(crate) limits: Interval<usize>,
    // Is applied to all voltages when traces are created
}

impl HasName for FlatBucketBlock {
    fn get_name(&self) -> &str {
        &self.name
    }
}

impl Debug for FlatBucketBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlatBucketBlock")
            .field("buckets", &self.buckets)
            .field("limits", &self.limits)
            .finish()
    }
}

impl FlatBucketBlock {
    pub(crate) fn find_bucket_matching(
        &mut self,
        collection: &EventlistsCollection,
    ) -> Option<(usize, Option<&mut FlatBucket>)> {
        self.buckets
            .iter_mut()
            .enumerate()
            .find(|(_, bucket)| bucket.is_collection_in(collection))
            .map(|(index, bucket)| (index, (self.limits.max > bucket.count).then_some(bucket)))
    }
}

///
/// This struct is created from the configuration JSON file.
///
pub(crate) struct FlatBucket {
    span: SpanOnce,
    // Is applied to all voltages when traces are created
    pub(crate) criteria: FlatCriteria,
    // Is applied to all voltages when traces are created
    pub(crate) algorithm: FlatAlgorithm,
    // Is applied to all voltages when traces are created
    pub(crate) waveform: FlatWaveform,
    pub(crate) count: usize,
}

impl Spanned for FlatBucket {
    fn span(&self) -> &SpanOnce {
        &self.span
    }
}

impl SpannedMut for FlatBucket {
    fn span_mut(&mut self) -> &mut SpanOnce {
        &mut self.span
    }
}

impl SpannedAggregator for FlatBucket {
    fn span_init(&mut self) -> Result<(), SpanOnceError> {
        self.span.init(info_span!("Bucket"))
    }

    fn link_current_span<F: Fn() -> tracing::Span>(
        &self,
        aggregated_span_fn: F,
    ) -> Result<(), SpanOnceError> {
        let span = self.span.get()?.in_scope(aggregated_span_fn);
        span.follows_from(tracing::Span::current());
        Ok(())
    }

    fn end_span(&self) -> Result<(), SpanOnceError> {
        Ok(())
    }
}

impl Debug for FlatBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlatBucket")
            .field("criteria", &self.criteria)
            .field("algorithm", &self.algorithm)
            .field("waveform", &self.waveform)
            .field("count", &self.count)
            .finish()
    }
}

impl FlatBucket {
    pub(crate) fn increment_count(&mut self) {
        self.count += 1;
    }
    pub(crate) fn is_collection_in(&self, collection: &EventlistsCollection) -> bool {
        self.criteria
            .digitiser_ids
            .is_valid(collection.digitiser_id)
            && self
                .criteria
                .periods
                .is_valid(collection.metadata.period_number)
            && self
                .criteria
                .frames
                .is_valid(collection.metadata.frame_number)
            && collection
                .channels
                .iter()
                .any(|&channel| self.criteria.channels.is_valid(channel))
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use digital_muon_streaming_types::FrameMetadata;

    use crate::engine::values::ConstantFilter;

    use super::*;

    #[test]
    fn criteria_any() {
        let bucket = FlatBucket {
            span: Default::default(),
            criteria: FlatCriteria {
                periods: ConstantFilter::Any,
                frames: ConstantFilter::Any,
                channels: ConstantFilter::Any,
                digitiser_ids: ConstantFilter::Any,
            },
            algorithm: FlatAlgorithm::FixedThreshold {
                threshold: Default::default(),
                duration: Default::default(),
                cool_down: Default::default(),
            },
            waveform: FlatWaveform::Flat {
                width: Default::default(),
            },
            count: 0,
        };
        let collection = EventlistsCollection {
            span: Default::default(),
            digitiser_id: 0,
            metadata: FrameMetadata {
                timestamp: Utc::now(),
                period_number: 0,
                protons_per_pulse: 1,
                running: false,
                frame_number: 1,
                veto_flags: 0,
            },
            eventlists: Default::default(),
            channels: vec![0, 1]
        };
        assert!(bucket.is_collection_in(&collection));
    }

    #[test]
    fn criteria_periods() {
        let bucket_1 = FlatBucket {
            span: Default::default(),
            criteria: FlatCriteria {
                periods: ConstantFilter::Is(0),
                frames: ConstantFilter::Any,
                channels: ConstantFilter::Any,
                digitiser_ids: ConstantFilter::Any,
            },
            algorithm: FlatAlgorithm::FixedThreshold {
                threshold: Default::default(),
                duration: Default::default(),
                cool_down: Default::default(),
            },
            waveform: FlatWaveform::Flat {
                width: Default::default(),
            },
            count: 0,
        };
        let bucket_2 = FlatBucket {
            span: Default::default(),
            criteria: FlatCriteria {
                periods: ConstantFilter::Is(1),
                frames: ConstantFilter::Any,
                channels: ConstantFilter::Any,
                digitiser_ids: ConstantFilter::Any,
            },
            algorithm: FlatAlgorithm::FixedThreshold {
                threshold: Default::default(),
                duration: Default::default(),
                cool_down: Default::default(),
            },
            waveform: FlatWaveform::Flat {
                width: Default::default(),
            },
            count: 0,
        };
        let collection = EventlistsCollection {
            span: Default::default(),
            digitiser_id: 0,
            metadata: FrameMetadata {
                timestamp: Utc::now(),
                period_number: 0,
                protons_per_pulse: 1,
                running: false,
                frame_number: 1,
                veto_flags: 0,
            },
            eventlists: Default::default(),
            channels: vec![0, 1],
        };
        assert!(bucket_1.is_collection_in(&collection));
        assert!(!bucket_2.is_collection_in(&collection));
    }
}
