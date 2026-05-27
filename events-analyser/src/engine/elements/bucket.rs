use crate::{
    engine::{
        Flattenable, FlattenableWithIndex, Templates,
        elements::{
            algorithm::FlatAlgorithm,
            criteria::{Criteria, FlatCriteria},
            waveform::{self, FlatWaveform},
        },
    },
    eventlists::EventlistsCollection,
};
use serde::Deserialize;

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct BucketBlock {
    // Is applied to all voltages when traces are created
    pub(crate) name: String,
    // Is applied to all voltages when traces are created
    pub(crate) number: usize,
    // Is applied to all voltages when traces are created
    pub(crate) criteria: Criteria,
    // Is applied to all voltages when traces are created
    pub(crate) algorithm: String,
    // Is applied to all voltages when traces are created
    pub(crate) waveform: String,
    // Is applied to all voltages when traces are created
    pub(crate) min: Option<usize>,
    // Is applied to all voltages when traces are created
    pub(crate) max: Option<usize>,
}

impl Flattenable for BucketBlock {
    type Flat = FlatBucketBlock;
    type Library = Templates;
    type Error = String;

    fn flatten(&self, library: &Templates) -> Result<FlatBucketBlock, Self::Error> {
        let algorithm = library
            .get_algorithm(&self.algorithm)
            .ok_or_else(|| format!("Could not find algorithm template in bucket {}.", self.name))?;

        let waveform = library
            .get_waveform(&self.waveform)
            .ok_or_else(|| format!("Could not find waveform template in bucket {}.", self.name))?;

        let buckets = (0..self.number)
            .map(|index| {
                let criteria = self.criteria.flatten(library, index)?;
                let algorithm = algorithm.flatten(&library.arrays, index)?;
                let waveform = waveform.flatten(&library.arrays, index)?;
                Ok(FlatBucket {
                    criteria,
                    algorithm,
                    waveform,
                })
            })
            .collect::<Result<Vec<_>, Self::Error>>()?;
        Ok(FlatBucketBlock {
            name: self.name.clone(),
            buckets,
            min: self.min,
            max: self.max,
        })
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug)]
pub(crate) struct FlatBucketBlock {
    // Is applied to all voltages when traces are created
    pub(crate) name: String,
    // Is applied to all voltages when traces are created
    pub(crate) buckets: Vec<FlatBucket>,
    // Is applied to all voltages when traces are created
    pub(crate) min: Option<usize>,
    // Is applied to all voltages when traces are created
    pub(crate) max: Option<usize>,
}

impl FlatBucketBlock {
    pub(crate) fn find_bucket_matching(
        &self,
        collection: &EventlistsCollection,
    ) -> Option<(usize, &FlatBucket)> {
        self.buckets
            .iter()
            .enumerate()
            .find(|(_, bucket)| bucket.is_collection_in(collection))
    }
}

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug)]
pub(crate) struct FlatBucket {
    // Is applied to all voltages when traces are created
    pub(crate) criteria: FlatCriteria,
    // Is applied to all voltages when traces are created
    pub(crate) algorithm: FlatAlgorithm,
    // Is applied to all voltages when traces are created
    pub(crate) waveform: FlatWaveform,
}

impl FlatBucket {
    pub(crate) fn is_collection_in(&self, collection: &EventlistsCollection) -> bool {
        self.criteria
            .digitiser_ids
            .is_valid(collection.digitiser_id)
            || self
                .criteria
                .periods
                .is_valid(collection.metadata.period_number)
            || self
                .criteria
                .frames
                .is_valid(collection.metadata.frame_number)
            || collection
                .channels
                .iter()
                .any(|&channel| self.criteria.channels.is_valid(channel))
    }
}
