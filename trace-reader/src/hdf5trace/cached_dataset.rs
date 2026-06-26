use crate::hdf5trace::Error;
use hdf5::{Dataset, H5Type};

/// Encapsulates a dataset and an in-memory vector allowing the user to load data in chunk sizes of their choosing.
pub(crate) struct CachedDataset<T> {
    /// The underlying dataset the instance caches.
    dataset: Dataset,
    /// The number of elements in the dataset.
    num_elements: usize,
    /// The data currently cached.
    current_cached_data: Vec<T>,
    /// The index of the first cached element. Only set if data is currently cached.
    first_cached_index: Option<usize>,
    /// The amount of the dataset to cache before usage.
    cache_size: usize,
}

impl<T: H5Type> CachedDataset<T> {
    /// Creates a new instance from the dataset and optional cache size.
    /// If no cache size is given, the dataset's chunk size is used.
    pub(crate) fn new(dataset: Dataset, cache_size: Option<&usize>) -> Result<Self, Error> {
        let cache_size = if let Some(cache_size) = cache_size {
            *cache_size
        } else {
            dataset
                .chunk()
                .expect("Chunk sizes should be accessible, this should never fail.")
                .first()
                .copied()
                .ok_or_else(|| Error::DatasetScalar(dataset.name()))?
        };
        let num_elements = *dataset
            .shape()
            .first()
            .expect("Dataset should be vector, this should never fail.");

        Ok(Self {
            dataset,
            num_elements,
            current_cached_data: Vec::new(),
            first_cached_index: None,
            cache_size,
        })
    }

    /// Given an index, ensure the necessary data is in the cache.
    /// This should each time before the `get_element` method is used.
    ///
    /// This method is idempotent, so does nothing if the required index is already cached.
    ///
    /// # Parameters
    /// - index: the index to ensure is cached.
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

    /// Caches elements from the given index.
    ///
    /// # Parameters
    /// - new_first_cached_index: caches all dataset elements from this index to `new_first_cached_index + self.cache_size`.
    #[tracing::instrument(skip_all, fields(old_index = self.first_cached_index, first_index=new_first_cached_index))]
    pub fn cache_elements(&mut self, new_first_cached_index: usize) {
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
        let data_slice = self
            .dataset
            .read_slice_1d::<T, _>(slice_info)
            .expect("Slice should be in range, this should never fail.");
        self.current_cached_data = data_slice.into_iter().collect();
    }

    /// Gets the element at the given index.
    ///
    /// If the data has not been properly cached, the wrong value may be returned, or the method may panic.
    /// It is up to the caller to ensure the `ensure_elements_cached` method has been called.
    pub(crate) fn get_element(&self, index: usize) -> &T {
        self.current_cached_data
            .get(index % self.cache_size)
            .expect("Index should be in range, this should never fail.")
    }

    /// Returns the number of elements.
    pub(crate) fn get_num_elements(&self) -> usize {
        self.num_elements
    }
}
