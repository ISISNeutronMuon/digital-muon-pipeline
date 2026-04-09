//!
//! # Example
//!
//! The following example applies a smoothing window of length five to a raw
//! data stream.
//! Note that a [SmoothingWindow] outputs a [Stats] type, so we need to extract
//! the [Stats::mean] value to convert to a scalar stream.
//! ```rust
//!     let smoothed = raw
//!        .window(SmoothingWindow::new(5))
//!        .map(|(i, stats)| (i, stats.mean));
//! ```
//use crate::pulse_detection::window::SliceWindow;
mod layer;
mod traces;

use super::Real;
use crate::{channels::LayerProcessingSettings, pulse_detection::window::convolution_filter::ConvolutionFilter};
use layer::Layer;
use traces::{ConvolutionCache, DetailCoefficients};

fn downsample(input: &[Real], output: &mut [Real], padding: usize) {
    let size = input.len();
    for (i, o) in output
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

fn upsample(input: &[Real], output: &mut [Real], padding: usize) {
    for (i, value) in input.iter().enumerate() {
        *output
            .get_mut(2 * i + padding)
            .expect("Slice element should exist, this should never fail.") = *value;
    }
}

#[derive(Default, Clone)]
pub(crate) struct PyramidFilter {
    subdivide_smoothing: ConvolutionFilter,
    refinement_smoothing: ConvolutionFilter,
    pyramid_base: Layer,
}

impl PyramidFilter {
    pub(crate) fn new(
        layer_settings: Vec<LayerProcessingSettings>,
        refinement_smoothing: ConvolutionFilter,
        subdivide_smoothing: ConvolutionFilter,
    ) -> Self {
        let subdivide_padding = subdivide_smoothing.kernel_size() / 2;
        let refined_padding = refinement_smoothing.kernel_size() / 2;
        let pyramid_base = Layer::new(layer_settings, subdivide_padding, refined_padding);
        PyramidFilter {
            subdivide_smoothing,
            refinement_smoothing,
            pyramid_base,
        }
    }
    pub(crate) fn init_size(&mut self, size: usize) {
        self.pyramid_base.init_size(size);
    }

    pub(crate) fn apply_to_slice<'a>(&mut self, input: &[Real]) -> Option<&[Real]> {
        self.pyramid_base.process(input, &self.refinement_smoothing, &self.subdivide_smoothing);
        self.pyramid_base.rebuild(&self.refinement_smoothing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        channels::LayerProcessingSettings, pulse_detection::window::{
            SliceWindow, convolution_filter::KernelType, fft_inverse::FftInverse,
        }, test_data::{NUM_VALUES, VALUES}
    };
    use rustfft::num_complex::{Complex, ComplexFloat};

    #[test]
    fn test_downsample() {
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut output = vec![0.0; 8];
        downsample(input.as_slice(), output.as_mut_slice(), 3);
        for (out, exp) in Iterator::zip(
            output.into_iter(),
            [0.0, 0.0, 0.0, 1.0, 3.0, 0.0, 0.0, 0.0].into_iter(),
        ) {
            assert_eq!(out, exp);
        }

        let input = vec![1.0, 2.0, 4.0];
        let mut output = vec![0.0; 8];
        downsample(input.as_slice(), output.as_mut_slice(), 2);
        for (out, exp) in Iterator::zip(
            output.into_iter(),
            [0.0, 0.0, 1.0, 4.0, 0.0, 0.0].into_iter(),
        ) {
            assert_eq!(out, exp);
        }
    }

    #[test]
    fn test_upsample() {
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let mut output = vec![0.0; 10];
        upsample(input.as_slice(), output.as_mut_slice(), 1);
        for (out, exp) in Iterator::zip(
            output.into_iter(),
            [0.0, 1.0, 0.0, 2.0, 0.0, 3.0, 0.0, 4.0, 0.0].into_iter(),
        ) {
            assert_eq!(out, exp);
        }

        let input = vec![1.0, 2.0, 4.0];
        let mut output = vec![0.0; 10];
        upsample(input.as_slice(), output.as_mut_slice(), 2);
        for (out, exp) in Iterator::zip(
            output.into_iter(),
            [0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 4.0, 0.0, 0.0, 0.0].into_iter(),
        ) {
            assert_eq!(out, exp);
        }
    }

    #[test]
    fn test_gaussian_convolution() {
        //FIXME
        let alpha_coefs = vec![0.125, 0.5, 0.75, 0.5, 0.125];
        let support = vec![-2, -1, 0, 1, 2];
        let mut gamma_coefs = vec![0.0; 5];

        let fft = FftInverse::new(50, 4, support.clone(), Complex::recip);
        fft.apply_to_slice(alpha_coefs.as_slice(), gamma_coefs.as_mut_slice());
        
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(alpha_coefs));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(gamma_coefs));

        let mut pyramid = PyramidFilter::new(
            vec![
                LayerProcessingSettings {
                    denoise_threshold: Some(0.0001),
                    enhance_threshold_factor: Some((0.0001, 1.1)),
                    multiply_factor: Some(1.01)
                };
                5
            ],
            alpha,
            gamma,
        );
        pyramid.init_size(NUM_VALUES);
        let output = pyramid.apply_to_slice(&VALUES).unwrap();
        //println!("{VALUES:?}");
        //println!("{output:?}\n");
        /*match pyramid.pyramid_base {
            Layer::Apex => unreachable!(),
            Layer::Level(layer_level) => {
                println!("SR {:?}\n", layer_level.subdivided.raw);
                println!("SC {:?}\n", layer_level.subdivided.convolved);
                println!("RR {:?}\n", layer_level.refined.raw);
                println!("RC {:?}\n", layer_level.refined.convolved);
                println!("D  {:?}\n", layer_level.detail_coefficients.0);
                println!("\n",);
            },
        }*/
    }
}
