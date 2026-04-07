use super::Real;
use crate::pulse_detection::window::{SliceWindow, convolution_filter::ConvolutionFilter};
use num::Zero;
use std::{
    ops::{Deref, DerefMut, Range},
    os::unix::process,
};


#[derive(Default, Clone)]
pub(super) struct ConvolutionCache {
    pub(super) padding: usize,
    pub(super) raw: Vec<Real>,
    pub(super) convolved: Vec<Real>,
}

impl ConvolutionCache {
    pub(super) fn new(size: usize, padding: usize) -> Self {
        Self {
            padding,
            raw: vec![0.0; size + 2 * padding],
            convolved: vec![0.0; size],
        }
    }

    pub(super) fn convolve(&mut self, alpha: &ConvolutionFilter) {
        alpha.apply_to_slice(self.raw.as_slice(), self.convolved.as_mut_slice());
        

        println!("Convolving");
        println!("  input = {:?}", self.raw.as_slice());
        println!("  output = {:?}\n", self.convolved.as_slice());
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
pub(super) struct DetailCoefficients(pub(super) Vec<Real>);

impl DetailCoefficients {
    pub(super) fn new(size: usize) -> Self {
        Self(vec![0.0; size])
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
    use crate::pulse_detection::{
        iterators::{PaddingIterable, WindowIterable},
        window::{SliceWindow, convolution_filter::KernelType, fft_inverse::FftInverse},
    };
    use assert_approx_eq::assert_approx_eq;
    use digital_muon_common::Intensity;
}