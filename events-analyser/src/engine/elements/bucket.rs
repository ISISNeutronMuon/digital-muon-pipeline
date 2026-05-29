use std::ops::Deref;

use crate::{
    engine::{
        Flattenable, FlattenableWithIndex, Templates,
        elements::{
            algorithm::FlatAlgorithm,
            criteria::{Criteria, CriteriaError, FlatCriteria},
            waveform::FlatWaveform,
        },
        utils::{Interval, WithName, WithSource}, values::ValueError,
    },
    eventlists::EventlistsCollection,
};
use serde::Deserialize;
use thiserror::Error;

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
}


///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct BucketBlockTemplate {
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
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct BucketBlock {
    // Is applied to all voltages when traces are created
    pub(crate) criteria: WithSource<Criteria>,
    // Is applied to all voltages when traces are created
    #[serde(flatten)]
    pub(crate) templatable: BucketBlockTemplate,
}

impl Deref for BucketBlock {
    type Target = BucketBlockTemplate;

    fn deref(&self) -> &Self::Target {
        &self.templatable
    }
}

impl Flattenable<&Templates> for WithSource<WithName<BucketBlock>> {
    type Flat = WithName<FlatBucketBlock>;
    type Error = BucketError;

    fn flatten(&self, library: &Templates) -> Result<WithName<FlatBucketBlock>, Self::Error> {
        let template = library.get_bucket(self);
        let number = self.templatable
            .number
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.number.as_ref()))
            .ok_or_else(|| BucketError::NoNumber(self.get_source().into()))?;
        let algorithm = self.templatable
            .algorithm
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.algorithm.as_ref()))
            .ok_or_else(|| BucketError::NoAlgorithm(self.get_source().into()))?;
        let waveform = self.templatable
            .waveform
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.waveform.as_ref()))
            .ok_or_else(|| BucketError::NoWaveform(self.get_source().into()))?;
        let limits = self.templatable
            .limits
            .as_ref()
            .or_else(|| template.and_then(|tmplt| tmplt.limits.as_ref()))
            .ok_or_else(|| BucketError::NoLimits(self.get_source().into()))?;

        let algorithm = library
            .get_algorithm(&algorithm)
            .ok_or_else(|| BucketError::NoAlgorithm(algorithm.into()))?;

        let waveform = library
            .get_waveform(&waveform)
            .ok_or_else(|| BucketError::NoWaveform(waveform.into()))?;

        let buckets = (0..*number)
            .map(|index| {
                let criteria = self.criteria.flatten(library, index)?;
                let algorithm = algorithm.flatten(library.get_arrays(), index)?;
                let waveform = waveform.flatten(&library.arrays, index)?;
                Ok(FlatBucket {
                    criteria,
                    algorithm,
                    waveform,
                    count: Default::default()
                })
            })
            .collect::<Result<Vec<_>, Self::Error>>()?;
        Ok(WithName::<FlatBucketBlock> {
            name: self.name.clone(),
            value: FlatBucketBlock {
            buckets,
            limits: limits.clone(),
            }
        })
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone)]
pub(crate) struct FlatBucketBlock {
    // Is applied to all voltages when traces are created
    pub(crate) buckets: Vec<FlatBucket>,
    // Is applied to all voltages when traces are created
    pub(crate) limits: Interval<usize>,
    // Is applied to all voltages when traces are created
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
            .map(|(index, bucket)| (index,
                (self.limits.max > bucket.count)
                    .then_some(bucket)
            ))
    }

    pub(crate) fn are_buckets_full_enough(&self) -> bool {
        self.buckets.iter().all(|bucket|bucket.count >= self.limits.min)
    }

}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Clone)]
pub(crate) struct FlatBucket {
    // Is applied to all voltages when traces are created
    pub(crate) criteria: FlatCriteria,
    // Is applied to all voltages when traces are created
    pub(crate) algorithm: FlatAlgorithm,
    // Is applied to all voltages when traces are created
    pub(crate) waveform: FlatWaveform,
    pub(crate) count: usize,
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
            criteria: FlatCriteria { periods: ConstantFilter::Any, frames: ConstantFilter::Any, channels: ConstantFilter::Any, digitiser_ids: ConstantFilter::Any },
            algorithm: FlatAlgorithm::FixedThreshold { threshold: Default::default(), duration: Default::default(), cool_down: Default::default()} ,
            waveform: FlatWaveform::Flat { width: Default::default() },
            count: 0,
        };
        let collection = EventlistsCollection {
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
            channels: vec![0,1],
        };
        assert!(bucket.is_collection_in(&collection));
    }

    #[test]
    fn criteria_periods() {
        let bucket_1 = FlatBucket {
            criteria: FlatCriteria { periods: ConstantFilter::Is(0), frames: ConstantFilter::Any, channels: ConstantFilter::Any, digitiser_ids: ConstantFilter::Any },
            algorithm: FlatAlgorithm::FixedThreshold { threshold: Default::default(), duration: Default::default(), cool_down: Default::default()} ,
            waveform: FlatWaveform::Flat { width: Default::default() },
            count: 0,
        };
        let bucket_2 = FlatBucket {
            criteria: FlatCriteria { periods: ConstantFilter::Is(1), frames: ConstantFilter::Any, channels: ConstantFilter::Any, digitiser_ids: ConstantFilter::Any },
            algorithm: FlatAlgorithm::FixedThreshold { threshold: Default::default(), duration: Default::default(), cool_down: Default::default()} ,
            waveform: FlatWaveform::Flat { width: Default::default() },
            count: 0,
        };
        let collection = EventlistsCollection {
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
            channels: vec![0,1],
        };
        assert!(bucket_1.is_collection_in(&collection));
        assert!(!bucket_2.is_collection_in(&collection));
    }
}
