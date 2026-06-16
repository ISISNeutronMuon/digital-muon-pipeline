use hdf5::{Dataset, H5Type};
use ndarray::{Array1, Dim};

pub(crate) struct FullDataset<T> {
    num_elements: usize,
    data: Array1<T>,
}

impl<T: H5Type + PartialEq + Copy> FullDataset<T> {
    pub(crate) fn new(dataset: Dataset) -> Self {
        let data = dataset.read_1d().unwrap();
        let num_elements = data.len();
        Self {
            num_elements,
            data
        }
    }

    pub(crate) fn iter(&self) -> ndarray::iter::Iter<'_, T, Dim<[usize; 1]>> {
        self.data.iter()
    }

    pub(crate) fn get_element(&self, index: usize) -> &T {
        self.data
            .get(index)
            .unwrap()
    }

    pub(crate) fn get_num_elements(&self) -> usize {
        self.num_elements
    }

    pub(crate) fn find_index_of(&self, value: T) -> Option<usize> {
        self.data.iter().enumerate().find_map(|(i,v)|(value.eq(v)).then_some(i))
    }
}

pub(crate) struct CachedDataset<T> {
    label: &'static str,
    dataset: Dataset,
    num_elements: usize,
    current_cached_data: Vec<T>,
    first_cached_index: Option<usize>,
    cache_size: usize,
}

impl<T: H5Type> CachedDataset<T> {
    pub(crate) fn new(dataset: Dataset, label: &'static str, cache_size: Option<usize>) -> Self {
        let cache_size =
            cache_size.unwrap_or_else(|| dataset.chunk().unwrap().first().unwrap().to_owned());
        let num_elements = *dataset.shape().get(0).unwrap();
        Self {
            dataset,
            label,
            num_elements,
            current_cached_data: Vec::new(),
            first_cached_index: None,
            cache_size,
        }
    }

    #[tracing::instrument(skip_all, fields(field_type = self.label))]
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

    #[tracing::instrument(skip_all, fields(old_index = self.first_cached_index, first_index=new_first_cached_index, field_type = self.label))]
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
        let data_slice = self.dataset.read_slice_1d::<T, _>(slice_info).unwrap();
        self.current_cached_data = data_slice.into_iter().collect();
    }

    pub(crate) fn get_element(&self, index: usize) -> &T {
        self.current_cached_data
            .get(index % self.cache_size)
            .unwrap()
    }

    pub(crate) fn get_num_elements(&self) -> usize {
        self.num_elements
    }
}
