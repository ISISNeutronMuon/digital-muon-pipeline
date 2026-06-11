use hdf5::{Dataset, H5Type, dataset::ChunkInfo};
use tracing::{debug, info};

pub(crate) struct CachedDataset<T> {
    dataset: Dataset,
    num_elements: usize,
    current_cached_data: Vec<T>,
    first_cached_index: Option<usize>,
    cache_size: usize,
}

impl<T: H5Type> CachedDataset<T> {
    pub(crate) fn new(dataset: Dataset, cache_size: Option<usize>) -> Self {
        let cache_size =
            cache_size.unwrap_or_else(|| dataset.chunk().unwrap().first().unwrap().to_owned());
        let num_elements = *dataset.shape().get(0).unwrap();
        Self {
            dataset,
            num_elements,
            current_cached_data: Vec::new(),
            first_cached_index: None,
            cache_size,
        }
    }

    pub(crate) fn ensure_elements_cached(&mut self, index: usize) {
        let new_first_cached_index = self.cache_size * index.div_euclid(self.cache_size);
        if self
            .first_cached_index
            .as_ref()
            .is_some_and(|first_cached_index| *first_cached_index == new_first_cached_index)
        {
            return;
        }
        self.first_cached_index = Some(new_first_cached_index);

        let slice_info = hdf5::SliceOrIndex::SliceCount {
            start: new_first_cached_index,
            step: 1,
            count: 1,
            block: self.cache_size,
        };
        //debug!("Reading element into cache from {}.", index);
        let data_slice = self.dataset.read_slice_1d::<T, _>(slice_info).unwrap();
        //debug!("Finished reading, writing element.");
        self.current_cached_data = data_slice.into_iter().collect();
    }

    pub(crate) fn get_element(&self, index: usize) -> &T {
        self.current_cached_data
            .get(index % self.cache_size)
            .unwrap()
    }

    pub(crate) fn get_num_element(&self) -> usize {
        self.num_elements
    }
}
