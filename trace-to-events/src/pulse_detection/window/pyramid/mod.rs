//!
mod layer;
mod traces;

use super::Real;
use crate::{
    channels::LayerProcessingSettings,
    pulse_detection::window::convolution_filter::ConvolutionFilter,
};
use layer::Layer;
use traces::{ConvolutionCache, DetailCoefficients};

/// Encapsulates all state and cache for the pyramid smoothing algorithm.
#[derive(Default, Clone)]
pub(crate) struct PyramidFilter {
    /// Smoothing filter to apply after downsampling.
    subdivide_smoothing: ConvolutionFilter,
    /// Smoothing filter to apply after upsampling.
    refinement_smoothing: ConvolutionFilter,
    /// The first layer of the pyramid.
    pyramid_base: Layer,
}

impl PyramidFilter {
    /// Create a new pyramid filter from the given vector of settings.
    /// # Parameters
    /// - layer_settings: Vector of Layer Settings, in descending order, i.e. the settings for the apex appears at the front.
    /// - subdivide_smoothing: the smoothing filter to apply after downsampling.
    /// - refinement_smoothing: the smoothing filter to apply after upsampling.
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

    /// Initialises the pyramid to have the given base size, and propagate
    /// this value through the layers of the pyramid.
    ///
    /// # Parameters
    /// - size: the size from which to initialise the pyramid's layers.
    pub(crate) fn init_size(&mut self, size: usize) {
        self.pyramid_base.init_size(size);
    }

    /// Apply the pyramid smoothing algorithm to the given input slice.
    ///
    /// Note that sizes are not checked at runtime.
    ///
    /// # Parameters
    /// - input: a slice of length equal to the size of the pyramid's base.
    ///
    /// # Return
    /// A slice with the result of the smoothing algorithm.
    pub(crate) fn apply_to_slice(&mut self, input: &[Real]) -> &[Real] {
        self.pyramid_base
            .build(input, &self.refinement_smoothing, &self.subdivide_smoothing);
        self.pyramid_base.process();
        self.pyramid_base.rebuild(&self.refinement_smoothing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        channels::LayerProcessingSettings,
        pulse_detection::window::{
            SliceWindow, convolution_filter::KernelType, fft_inverse::FftInverse,
        },
        test_data::{
            assert_slices_equal,
            pyramid::{INPUT, layer1, layer2, layer3, layer4},
        },
    };
    use num::complex::ComplexFloat;

    #[test]
    fn test_pyramid() {
        let refinement_smoothing_coefs = vec![0.125, 0.5, 0.75, 0.5, 0.125];
        let support = vec![-2, -1, 0, 1, 2];
        let mut subdivide_smoothing_coefs = vec![0.0; 5];

        let fft = FftInverse::new(200, 5, support.clone(), ComplexFloat::recip);
        fft.apply_to_slice(
            refinement_smoothing_coefs.as_slice(),
            subdivide_smoothing_coefs.as_mut_slice(),
        );

        assert_slices_equal(
            &subdivide_smoothing_coefs,
            &[0.04112906, -0.23971773, 1.39717735, -0.23971773, 0.04112906],
        );

        let layer_processing_settings = vec![
            LayerProcessingSettings {
                denoise_threshold: Some(0.002),
                enhance_threshold_factor: Some((0.004, 1.5)),
                multiply_factor: Some(1.0),
            },
            LayerProcessingSettings {
                denoise_threshold: Some(0.005),
                enhance_threshold_factor: Some((0.003, 1.375)),
                multiply_factor: Some(0.7),
            },
            LayerProcessingSettings {
                denoise_threshold: Some(0.007),
                enhance_threshold_factor: Some((0.0035, 1.25)),
                multiply_factor: Some(0.2),
            },
            LayerProcessingSettings {
                denoise_threshold: Some(0.02),
                enhance_threshold_factor: Some((0.005, 1.125)),
                multiply_factor: Some(0.1),
            },
        ];
        let refinement_smoothing =
            ConvolutionFilter::new(KernelType::ManualCoefficients(refinement_smoothing_coefs));
        let subdivide_smoothing =
            ConvolutionFilter::new(KernelType::ManualCoefficients(subdivide_smoothing_coefs));
        let mut pyramid = PyramidFilter::new(
            layer_processing_settings,
            refinement_smoothing,
            subdivide_smoothing,
        )
        .unwrap();
        pyramid.init_size(128);

        pyramid.pyramid_base.build(
            &INPUT,
            &pyramid.refinement_smoothing,
            &pyramid.subdivide_smoothing,
        );
        {
            let layer_1 = &pyramid.pyramid_base;
            let layer_2 = layer_1.get_next_layer().unwrap();
            let layer_3 = layer_2.get_next_layer().unwrap();
            let layer_4 = layer_3.get_next_layer().unwrap();

            // Test subdivided
            assert_slices_equal(layer_4.get_subdivided(), &layer4::SUBDIVIDED);
            assert_slices_equal(layer_3.get_subdivided(), &layer3::SUBDIVIDED);
            assert_slices_equal(layer_2.get_subdivided(), &layer2::SUBDIVIDED);
            assert_slices_equal(layer_1.get_subdivided(), &layer1::SUBDIVIDED);

            // Test refined
            assert_slices_equal(layer_4.get_refined(), &layer4::REFINED);
            assert_slices_equal(layer_3.get_refined(), &layer3::REFINED);
            assert_slices_equal(layer_2.get_refined(), &layer2::REFINED);
            assert_slices_equal(layer_1.get_refined(), &layer1::REFINED);

            // Test detail_coefficients before processing
            assert_slices_equal(
                layer_4.get_detail_coefficients(),
                &layer4::DETAIL_COEFFICIENTS,
            );
            assert_slices_equal(
                layer_3.get_detail_coefficients(),
                &layer3::DETAIL_COEFFICIENTS,
            );
            assert_slices_equal(
                layer_2.get_detail_coefficients(),
                &layer2::DETAIL_COEFFICIENTS,
            );
            assert_slices_equal(
                layer_1.get_detail_coefficients(),
                &layer1::DETAIL_COEFFICIENTS,
            );
        }

        pyramid.pyramid_base.process();
        {
            let layer_1 = &pyramid.pyramid_base;
            let layer_2 = layer_1.get_next_layer().unwrap();
            let layer_3 = layer_2.get_next_layer().unwrap();
            let layer_4 = layer_3.get_next_layer().unwrap();

            // Test detail_coefficients after processing
            assert_slices_equal(
                layer_4.get_detail_coefficients(),
                &layer4::NEW_DETAIL_COEFFICIENTS,
            );
            assert_slices_equal(
                layer_3.get_detail_coefficients(),
                &layer3::NEW_DETAIL_COEFFICIENTS,
            );
            assert_slices_equal(
                layer_2.get_detail_coefficients(),
                &layer2::NEW_DETAIL_COEFFICIENTS,
            );
            assert_slices_equal(
                layer_1.get_detail_coefficients(),
                &layer1::NEW_DETAIL_COEFFICIENTS,
            );
        }

        pyramid.pyramid_base.rebuild(&pyramid.refinement_smoothing);
        {
            let layer_1 = &pyramid.pyramid_base;
            let layer_2 = layer_1.get_next_layer().unwrap();
            let layer_3 = layer_2.get_next_layer().unwrap();
            let layer_4 = layer_3.get_next_layer().unwrap();

            // Test rebuilt
            assert_slices_equal(layer_4.get_refined(), &layer4::REBUILT);
            assert_slices_equal(layer_3.get_refined(), &layer3::REBUILT);
            assert_slices_equal(layer_2.get_refined(), &layer2::REBUILT);
            assert_slices_equal(layer_1.get_refined(), &layer1::REBUILT);
        }
    }
}
