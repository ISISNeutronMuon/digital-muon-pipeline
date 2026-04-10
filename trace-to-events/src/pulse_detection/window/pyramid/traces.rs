use super::Real;
use crate::pulse_detection::window::{SliceWindow, convolution_filter::ConvolutionFilter};
use std::ops::{AddAssign, Deref, DerefMut};

/// 
#[derive(Default, Clone)]
pub(super) struct ConvolutionCache {
    /// The amount of extra space to allocate to the beginning and end of the raw vector.
    padding: usize,
    /// The memory that the convolution operator reads from.
    raw: Vec<Real>,
    /// The memory that the convolution operator writes to.
    convolved: Vec<Real>,
}

impl ConvolutionCache {
    /// 
    pub(super) fn new(padding: usize) -> Self {
        Self {
            padding,
            raw: Default::default(),
            convolved: Default::default(),
        }
    }

    pub(super) fn init_size(&mut self, size: usize) {
        self.raw.resize(size + 2 * self.padding, Default::default());
        self.convolved.resize(size, Default::default());
    }

    pub(super) fn get_padding(&self) -> usize {
        self.padding
    }

    pub(super) fn convolve(&mut self, alpha: &ConvolutionFilter) {
        alpha.apply_to_slice(self.raw.as_slice(), self.convolved.as_mut_slice());
    }

    pub(super) fn append_slice(&mut self, detail_coefficients: &[Real]) {
        self.convolved
            .iter_mut()
            .zip(detail_coefficients.iter())
            .for_each(|(coef, det)|coef.add_assign(det));
    }

    pub(super) fn sum_from_slices(&mut self, refined: &[Real], detail_coefficients: &[Real]) {
        let iters = Iterator::zip(refined.iter(), detail_coefficients.iter());
        self.convolved
            .iter_mut()
            .zip(iters)
            .for_each(|(coef, (rfn, det))| *coef = *rfn + *det);
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
pub(super) struct DetailCoefficients(Vec<Real>);

impl DetailCoefficients {
    pub(super) fn new() -> Self {
        Self(Default::default())
    }

    pub(super) fn init_size(&mut self, size: usize) {
        self.0.resize(size, Default::default());
    }

    pub(super) fn denoise(&mut self, threshold: Real) {
        self.0
            .iter_mut()
            .filter(|val| val.abs() < threshold)
            .for_each(|val| *val = Default::default());
    }

    pub(super) fn enhance(&mut self, threshold: Real, factor: Real) {
        self.0
            .iter_mut()
            .filter(|val| val.abs() > threshold)
            .for_each(|val| *val *= factor);
    }

    pub(super) fn multiply(&mut self, factor: Real) {
        self.0.iter_mut().for_each(|val| *val *= factor);
    }

    pub(super) fn extract_from_slices(&mut self, source: &[Real], refined: &[Real]) {
        let iters = Iterator::zip(source.iter(), refined.iter());
        self.0.iter_mut()
            .zip(iters)
            .for_each(|(coef, (src, rfn))| *coef = *src - *rfn);
    }
}

impl Deref for DetailCoefficients {
    type Target = [Real];

    fn deref(&self) -> &Self::Target {
        self.0.as_slice()
    }
}

impl DerefMut for DetailCoefficients {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut_slice()
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
}
