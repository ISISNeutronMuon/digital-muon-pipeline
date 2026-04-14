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
use crate::{
    channels::LayerProcessingSettings,
    pulse_detection::window::convolution_filter::ConvolutionFilter,
};
use layer::Layer;
use traces::{ConvolutionCache, DetailCoefficients};

/// Applies the pyramid filtering algorithm by [TODO].
#[derive(Default, Clone)]
pub(crate) struct PyramidFilter {
    subdivide_smoothing: ConvolutionFilter,
    refinement_smoothing: ConvolutionFilter,
    pyramid_base: Layer,
}

impl PyramidFilter {
    /// Create a new pyramid filter from the given vector of settings.
    /// # Parameters
    /// - layer_settings: Vector of Layer Settings, descending from the top layer.
    pub(crate) fn new(
        mut layer_settings: Vec<LayerProcessingSettings>,
        refinement_smoothing: ConvolutionFilter,
        subdivide_smoothing: ConvolutionFilter,
    ) -> Option<Self> {
        let subdivide_padding = subdivide_smoothing.kernel_size() / 2;
        let refined_padding = refinement_smoothing.kernel_size() / 2;

        layer_settings.pop().map(|first_layer_settings| {
            let pyramid_base = Layer::new(
                first_layer_settings,
                subdivide_padding,
                refined_padding,
                layer_settings,
            );
            PyramidFilter {
                subdivide_smoothing,
                refinement_smoothing,
                pyramid_base,
            }
        })
    }
    pub(crate) fn init_size(&mut self, size: usize) {
        self.pyramid_base.init_size(size);
    }

    pub(crate) fn apply_to_slice<'a>(&mut self, input: &[Real]) -> &[Real] {
        self.pyramid_base
            .build(input, &self.refinement_smoothing, &self.subdivide_smoothing);
        self.pyramid_base
            .process();
        self.pyramid_base.rebuild(&self.refinement_smoothing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        channels::LayerProcessingSettings,
        pulse_detection::window::{
            SliceWindow, convolution_filter::KernelType, fft_inverse::FftInverse
        },
        test_data::{assert_iters_equal, pyramid},
    };
    use num::complex::ComplexFloat;

    #[test]
    fn test_pyramid() {
        let refinement_smoothing_coefs = vec![0.125, 0.5, 0.75, 0.5, 0.125];
        let support = vec![-2, -1, 0, 1, 2];
        let mut subdivide_smoothing_coefs = vec![0.0; 5];

        let fft = FftInverse::new(200, 5, support.clone(), ComplexFloat::recip);
        fft.apply_to_slice(refinement_smoothing_coefs.as_slice(), subdivide_smoothing_coefs.as_mut_slice());

        assert_iters_equal(subdivide_smoothing_coefs.iter(), [ 0.04112906, -0.23971773,  1.39717735, -0.23971773,  0.04112906].iter());

        let layer_processing_settings = vec![
            LayerProcessingSettings {
                denoise_threshold: Some(0.002),
                enhance_threshold_factor: Some((0.004,1.5)),
                multiply_factor: Some(1.0)
            },
            LayerProcessingSettings {
                denoise_threshold: Some(0.005),
                enhance_threshold_factor: Some((0.003,1.375)),
                multiply_factor: Some(0.7)
            },
            LayerProcessingSettings {
                denoise_threshold: Some(0.007),
                enhance_threshold_factor: Some((0.0035,1.25)),
                multiply_factor: Some(0.2)
            },
            LayerProcessingSettings {
                denoise_threshold: Some(0.02),
                enhance_threshold_factor: Some((0.005,1.125)),
                multiply_factor: Some(0.1)
            }
        ];
        let refinement_smoothing = ConvolutionFilter::new(KernelType::ManualCoefficients(refinement_smoothing_coefs));
        let subdivide_smoothing = ConvolutionFilter::new(KernelType::ManualCoefficients(subdivide_smoothing_coefs));
        let mut pyramid = PyramidFilter::new(layer_processing_settings,refinement_smoothing,subdivide_smoothing).unwrap();
        pyramid.init_size(128);

        pyramid.pyramid_base.build(&pyramid::INPUT, &pyramid.refinement_smoothing, &pyramid.subdivide_smoothing);
        {
            let layer_1 = &pyramid.pyramid_base;
            let layer_2 = layer_1.get_next_layer().unwrap();
            let layer_3 = layer_2.get_next_layer().unwrap();
            let layer_4 = layer_3.get_next_layer().unwrap();

            // Test subdivided
            assert_iters_equal(layer_4.get_subdivided().iter(), pyramid::DECIMATED_4.iter());
            assert_iters_equal(layer_3.get_subdivided().iter(), pyramid::DECIMATED_3.iter());
            assert_iters_equal(layer_2.get_subdivided().iter(), pyramid::DECIMATED_2.iter());
            assert_iters_equal(layer_1.get_subdivided().iter(), pyramid::DECIMATED_1.iter());
            
            // Test refined
            assert_iters_equal(layer_4.get_refined().iter(), pyramid::REFINED_4.iter());
            assert_iters_equal(layer_3.get_refined().iter(), pyramid::REFINED_3.iter());
            assert_iters_equal(layer_2.get_refined().iter(), pyramid::REFINED_2.iter());
            assert_iters_equal(layer_1.get_refined().iter(), pyramid::REFINED_1.iter());

            // Test detail_coefficients before processing
            assert_iters_equal(layer_4.get_detail_coefficients().iter(),pyramid::DETAIL_COEFFICIENTS_4.iter());
            assert_iters_equal(layer_3.get_detail_coefficients().iter(), pyramid::DETAIL_COEFFICIENTS_3.iter());
            assert_iters_equal(layer_2.get_detail_coefficients().iter(), pyramid::DETAIL_COEFFICIENTS_2.iter());
            assert_iters_equal(layer_1.get_detail_coefficients().iter(), pyramid::DETAIL_COEFFICIENTS_1.iter());
        }
        
        pyramid.pyramid_base.process();
        {
            let layer_1 = &pyramid.pyramid_base;
            let layer_2 = layer_1.get_next_layer().unwrap();
            let layer_3 = layer_2.get_next_layer().unwrap();
            let layer_4 = layer_3.get_next_layer().unwrap();

            // Test detail_coefficients after processing
            assert_iters_equal(layer_4.get_detail_coefficients().iter(), pyramid::NEW_DETAIL_COEFFICIENTS_4.iter());
            assert_iters_equal(layer_3.get_detail_coefficients().iter(), pyramid::NEW_DETAIL_COEFFICIENTS_3.iter());
            assert_iters_equal(layer_2.get_detail_coefficients().iter(), pyramid::NEW_DETAIL_COEFFICIENTS_2.iter());
            assert_iters_equal(layer_1.get_detail_coefficients().iter(), pyramid::NEW_DETAIL_COEFFICIENTS_1.iter());
        }

        pyramid.pyramid_base.rebuild(&pyramid.refinement_smoothing);
        {
            let layer_1 = &pyramid.pyramid_base;
            let layer_2 = layer_1.get_next_layer().unwrap();
            let layer_3 = layer_2.get_next_layer().unwrap();
            let layer_4 = layer_3.get_next_layer().unwrap();

            // Test rebuilt
            assert_iters_equal(layer_4.get_rebuilt().iter(), pyramid::REBUILT_4.iter());
            assert_iters_equal(layer_3.get_rebuilt().iter(), pyramid::REBUILT_3.iter());
            assert_iters_equal(layer_2.get_rebuilt().iter(), pyramid::REBUILT_2.iter());
            assert_iters_equal(layer_1.get_rebuilt().iter(), pyramid::REBUILT_1.iter());
        }

    }
}
