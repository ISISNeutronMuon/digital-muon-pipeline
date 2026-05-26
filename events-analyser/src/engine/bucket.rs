use crate::{
    engine::{
        Flattenable, FlattenableWithIndex, Templates,
        algorithm::FlatAlgorithm,
        criteria::{Criteria, FlatCriteria},
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
            .algorithms
            .iter()
            .find(|alg| alg.has_name(&self.algorithm))
            .ok_or_else(|| format!("Could not find algorithm template in bucket {}.", self.name))?;
        let buckets = (0..self.number)
            .map(|index| {
                let criteria = self.criteria.flatten(library, index)?;
                let algorithm = algorithm.get_algorithm().flatten(&library.arrays, index)?;
                Ok(FlatBucket {
                    criteria,
                    algorithm,
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

///
/// This struct is created from the configuration JSON file.
///
#[derive(Debug)]
pub(crate) struct FlatBucket {
    // Is applied to all voltages when traces are created
    pub(crate) criteria: FlatCriteria,
    // Is applied to all voltages when traces are created
    pub(crate) algorithm: FlatAlgorithm,
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
