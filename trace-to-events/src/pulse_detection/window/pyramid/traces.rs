use crate::{
    channels::LayerProcessingSettings,
    pulse_detection::{
        Real,
        window::{SliceWindow, convolution_filter::ConvolutionFilter},
    },
};
use std::ops::{AddAssign, Deref, DerefMut};

/// A pair of vectors designed to work with [ConvolutionFilter] and the pyramid smoothing algorithm.
///
/// The struct is created with a `padding` value, and consists of two `Vec`s: `raw` and `convolved`.
/// Before the struct can be used, it should be initialised with `Self::init_size` which sets the sizes
/// of `convolved` and `raw`, in which `raw` has `padding` many elements prepended and appended.
///
/// The expected use cases for [ConvolutionCache] consist of:
/// - When mutably dereferenced, [ConvolutionCache] returns a mut slice to [Self::raw], which the caller can write directly to,
///   (alternatively the caller may use helper methods [Self::downsample] and [Self::downsample]).
/// - The convolution is then applied by calling [Self::convolve], the result of which is written to [Self::convolved].
/// - The results of the convolution are read by immutably dereferencing [ConvolutionCache], which returns an immutable slice to [Self::convolved].
///
/// # Example
/// ```rust
/// let mut cache = ConvolutionCache::new(10);
/// cache.init_size(100);
/// write_stuff_to_vec(&mut cache);
/// let convolution : ConvolutionFilter = /* some filter */;
/// cache.convolve(&convolution)
/// do_something_with_vec(&cache);
/// ```
#[derive(Default, Clone)]
pub(super) struct ConvolutionCache {
    /// The amount of extra space to allocate to the beginning and end of the raw vector.
    padding: usize,
    /// The preconvolution memory block, the caller should write to this by mutably dereferencing the object.
    raw: Vec<Real>,
    /// The postconvolution memory block, the caller should read from this by immutably dereferencing the object.
    convolved: Vec<Real>,
}

impl ConvolutionCache {
    /// Create new Convolution with the given padding added to the front and end of the `raw` `Vec`.
    pub(super) fn new(padding: usize) -> Self {
        Self {
            padding,
            raw: Default::default(),
            convolved: Default::default(),
        }
    }

    /// Initialise the memory blocks with the given size.
    pub(super) fn init_size(&mut self, size: usize) {
        self.raw.resize(size + 2 * self.padding, Default::default());
        self.convolved.resize(size, Default::default());
    }

    // Helper Methods.
    // Note these modify the `convolved` `Vec` directly, something the caller is prevented from doing.

    /// Apply the given convolution to the object, reading from the `raw` `Vec` and writing to `convolved`.
    pub(super) fn convolve(&mut self, convolution: &ConvolutionFilter) {
        convolution.apply_to_slice(self.raw.as_slice(), self.convolved.as_mut_slice());
    }

    /// Adds a slice of values elementwise to the current convolved values.
    ///
    /// # Parameters
    /// - detail_coefficients: the slice whose elements to sum.
    pub(super) fn inject_details(&mut self, detail_coefficients: &DetailCoefficients) {
        self.convolved
            .iter_mut()
            .zip(detail_coefficients.iter())
            .for_each(|(coef, det)| coef.add_assign(det));
    }

    /// Given a slice of size `2*convolved.len()`, this method downsamples `input` to [Self::raw]
    /// by sampling only the even indices of `input`.
    ///
    /// Values of index `2*i` in `input` are written to index `i + padding` in [Self::raw].
    /// This ensures [Self::raw] is left and right padded with `padding` zeroes (assuming they were zero previously).
    ///
    /// The size of `input` is not checked, and is assumed to be of size at least `2*convolved.len()`.
    ///
    /// # Parameters
    /// - input: the slice from which the downsample.
    pub(super) fn downsample(&mut self, input: &[Real]) {
        let size = input.len();
        let padding = self.padding;
        for (i, o) in self
            .iter_mut()
            .skip(padding)
            .take(size.div_ceil(2))
            .enumerate()
        {
            *o = *input
                .get(2 * i)
                .expect("Slice element should exist, this should never fail.");
        }
    }

    /// Given a slice of size `convolved.len()/2`, this method upsamples `input` to [Self::raw]
    /// by distributing `input` to the even indices of [Self::raw].
    ///
    /// Values of index `i` in `input` are written to index `2*i + padding` in [Self::raw].
    /// This ensures [Self::raw] is left and right padded with `padding` zeroes (assuming they were zero previously).
    ///
    /// The size of `input` is not checked, and is assumed to be of size at most `convolved.len()/2`.
    ///
    /// # Parameters
    /// - input: the slice from which the downsample.
    pub(super) fn upsample(&mut self, input: &[Real]) {
        let padding = self.padding;
        for (i, value) in input.iter().enumerate() {
            *self
                .deref_mut()
                .get_mut(2 * i + padding)
                .expect("Slice element should exist, this should never fail.") = *value;
        }
    }
}

impl Deref for ConvolutionCache {
    type Target = [Real];

    fn deref(&self) -> &Self::Target {
        self.convolved.as_slice()
    }
}

impl DerefMut for ConvolutionCache {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.raw.as_mut_slice()
    }
}

#[derive(Default, Clone)]
pub(super) struct DetailCoefficients {
    settings: LayerProcessingSettings,
    details: Vec<Real>,
}

impl DetailCoefficients {
    pub(super) fn new(settings: LayerProcessingSettings) -> Self {
        Self {
            settings,
            ..Default::default()
        }
    }

    pub(super) fn init_size(&mut self, size: usize) {
        self.details.resize(size, Default::default());
    }

    pub(super) fn process(&mut self) {
        if let Some(denoise_threshold) = self.settings.denoise_threshold {
            self.details
                .iter_mut()
                .filter(|val| val.abs() < denoise_threshold)
                .for_each(|val| *val = Default::default());
        }
        if let Some((enhance_threshold, enhance_factor)) = self.settings.enhance_threshold_factor {
            self.details
                .iter_mut()
                .filter(|val| **val > enhance_threshold)
                .for_each(|val| *val *= enhance_factor);
        }
        if let Some(multiply_factor) = self.settings.multiply_factor {
            self.details
                .iter_mut()
                .for_each(|val| *val *= multiply_factor);
        }
    }

    pub(super) fn extract_from_slices(&mut self, source: &[Real], refined: &[Real]) {
        let iters = Iterator::zip(source.iter(), refined.iter());
        self.details
            .iter_mut()
            .zip(iters)
            .for_each(|(coef, (src, rfn))| *coef = *src - *rfn);
    }
}

impl Deref for DetailCoefficients {
    type Target = [Real];

    fn deref(&self) -> &Self::Target {
        self.details.as_slice()
    }
}

impl DerefMut for DetailCoefficients {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.details.as_mut_slice()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_convolved_cache_sizes() {
        let mut cache = ConvolutionCache::new(5);
        assert_eq!(cache.raw.len(), 0);
        assert_eq!(cache.convolved.len(), 0);
        cache.init_size(15);
        assert_eq!(cache.raw.len(), 25);
        assert_eq!(cache.convolved.len(), 15);
    }

    #[test]
    fn test_downsample() {
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut cache = ConvolutionCache::new(3);
        cache.init_size(2);
        cache.downsample(input.as_slice());
        for (out, exp) in Iterator::zip(
            cache.as_mut().iter_mut(),
            [0.0, 0.0, 0.0, 1.0, 3.0, 0.0, 0.0, 0.0].into_iter(),
        ) {
            assert_eq!(*out, exp);
        }

        let input = vec![1.0, 2.0, 4.0];
        let mut cache = ConvolutionCache::new(2);
        cache.init_size(2);
        cache.downsample(input.as_slice());
        for (out, exp) in Iterator::zip(
            cache.as_mut().iter_mut(),
            [0.0, 0.0, 1.0, 4.0, 0.0, 0.0].into_iter(),
        ) {
            assert_eq!(*out, exp);
        }
    }

    #[test]
    fn test_upsample() {
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut cache = ConvolutionCache::new(1);
        cache.init_size(6);
        cache.upsample(input.as_slice());
        for (out, exp) in Iterator::zip(
            cache.as_mut().iter_mut(),
            [0.0, 1.0, 0.0, 2.0, 0.0, 3.0, 0.0, 4.0, 0.0, 0.0].into_iter(),
        ) {
            assert_eq!(*out, exp);
        }

        let input = vec![1.0, 2.0, 4.0];
        let mut cache = ConvolutionCache::new(2);
        cache.init_size(6);
        cache.upsample(input.as_slice());
        for (out, exp) in Iterator::zip(
            cache.as_mut().iter_mut(),
            [0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 4.0, 0.0, 0.0, 0.0].into_iter(),
        ) {
            assert_eq!(*out, exp);
        }
    }

    #[test]
    fn assert_layer_settings_default() {
        let details = DetailCoefficients::default();
        assert!(details.settings.denoise_threshold.is_none());
        assert!(details.settings.enhance_threshold_factor.is_none());
        assert!(details.settings.multiply_factor.is_none());
    }
}
