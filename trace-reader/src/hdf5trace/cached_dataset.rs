use crate::hdf5trace::Error;
use hdf5::{Dataset, H5Type};

/// 
pub(crate) struct CachedDataset<T> {
    dataset: Dataset,
    num_elements: usize,
    current_cached_data: Vec<T>,
    first_cached_index: Option<usize>,
    cache_size: usize,
}

impl<T: H5Type> CachedDataset<T> {
    pub(crate) fn new(dataset: Dataset, cache_size: Option<&usize>) -> Result<Self, Error> {
        let cache_size = if let Some(cache_size) = cache_size {
            *cache_size
        } else {
            dataset.chunk()
                .expect("Chunk sizes should be accessible, this should never fail.")
                .first()
                .copied()
                .ok_or_else(||Error::DatasetScalar(dataset.name()))?
        };
        let num_elements = *dataset.shape().first().expect("This should never fail.");
        
        Ok(Self {
            dataset,
            num_elements,
            current_cached_data: Vec::new(),
            first_cached_index: None,
            cache_size,
        })
    }

    #[tracing::instrument(skip_all)]
    pub(crate) fn ensure_elements_cached(&mut self, index: usize) {
        let new_first_cached_index = self.cache_size * index.div_euclid(self.cache_size);
        if self
            .first_cached_index
            .as_ref()
            .is_none_or(|first_cached_index| *first_cached_index != new_first_cached_index)
        {
            self.cache_elements(new_first_cached_index);
        }
    }

    #[tracing::instrument(skip_all, fields(old_index = self.first_cached_index, first_index=new_first_cached_index))]
    pub(crate) fn cache_elements(&mut self, new_first_cached_index: usize) {
        self.first_cached_index = Some(new_first_cached_index);

        let size = if new_first_cached_index + self.cache_size > self.num_elements {
            self.num_elements - new_first_cached_index
        } else {
            self.cache_size
        };
        let slice_info = hdf5::SliceOrIndex::SliceCount {
            start: new_first_cached_index,
            step: 1,
            count: 1,
            block: size,
        };
        let data_slice = self.dataset.read_slice_1d::<T, _>(slice_info)
            .expect("Slice should be in range, this should never fail.");
        self.current_cached_data = data_slice.into_iter().collect();
    }

    pub(crate) fn get_element(&self, index: usize) -> &T {
        self.current_cached_data
            .get(index % self.cache_size)
            .expect("Index should be in range, this should never fail.")
    }

    pub(crate) fn get_num_elements(&self) -> usize {
        self.num_elements
    }
}
