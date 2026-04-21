//! The pyramid filter window smooths a trace signal by downscaling, upscaling, and smoothing,
//! over several different resolutions number of times. This allows noise at different
//! frequencies to be isolated and removed before a different detection algorithm is applied.
mod layer;
mod traces;

pub(crate) use layer::PyramidLayer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        channels::LayerProcessingSettings,
        pulse_detection::window::{
            SliceWindow,
            convolution_filter::{ConvolutionFilter, KernelType},
            fft_inverse::FftInverse,
        },
        test_data::{
            assert_slices_approx_equal,
            pyramid::{INPUT, layer1, layer2, layer3, layer4},
        },
    };
    use num::complex::ComplexFloat;

    #[test]
    fn test_pyramid() {
        let upsample_smoothing_coefs = vec![0.125, 0.5, 0.75, 0.5, 0.125];
        let support = vec![-2, -1, 0, 1, 2];
        let mut downsample_smoothing_coefs = vec![0.0; 5];

        let fft = FftInverse::new(200, 5, support.clone(), ComplexFloat::recip);
        fft.apply_to_slice(
            upsample_smoothing_coefs.as_slice(),
            downsample_smoothing_coefs.as_mut_slice(),
        );

        assert_slices_approx_equal(
            &downsample_smoothing_coefs,
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
        let upsample_smoothing =
            ConvolutionFilter::new(KernelType::ManualCoefficients(upsample_smoothing_coefs));
        let downsample_smoothing =
            ConvolutionFilter::new(KernelType::ManualCoefficients(downsample_smoothing_coefs));

        let mut pyramid_base = PyramidLayer::new(
            layer_processing_settings,
            downsample_smoothing.kernel_size() / 2,
            upsample_smoothing.kernel_size() / 2,
        )
        .unwrap();
        pyramid_base.init_size(128);

        pyramid_base.build(&INPUT, &downsample_smoothing, &upsample_smoothing);
        {
            let layer_1 = &pyramid_base;
            let layer_2 = layer_1.get_next_layer().unwrap();
            let layer_3 = layer_2.get_next_layer().unwrap();
            let layer_4 = layer_3.get_next_layer().unwrap();

            // Test subdivided
            assert_slices_approx_equal(layer_4.get_subdivided(), &layer4::SUBDIVIDED);
            assert_slices_approx_equal(layer_3.get_subdivided(), &layer3::SUBDIVIDED);
            assert_slices_approx_equal(layer_2.get_subdivided(), &layer2::SUBDIVIDED);
            assert_slices_approx_equal(layer_1.get_subdivided(), &layer1::SUBDIVIDED);

            // Test refined
            assert_slices_approx_equal(layer_4.get_refined(), &layer4::REFINED);
            assert_slices_approx_equal(layer_3.get_refined(), &layer3::REFINED);
            assert_slices_approx_equal(layer_2.get_refined(), &layer2::REFINED);
            assert_slices_approx_equal(layer_1.get_refined(), &layer1::REFINED);

            // Test detail_coefficients before processing
            assert_slices_approx_equal(
                layer_4.get_detail_coefficients(),
                &layer4::DETAIL_COEFFICIENTS,
            );
            assert_slices_approx_equal(
                layer_3.get_detail_coefficients(),
                &layer3::DETAIL_COEFFICIENTS,
            );
            assert_slices_approx_equal(
                layer_2.get_detail_coefficients(),
                &layer2::DETAIL_COEFFICIENTS,
            );
            assert_slices_approx_equal(
                layer_1.get_detail_coefficients(),
                &layer1::DETAIL_COEFFICIENTS,
            );
        }

        pyramid_base.process();
        {
            let layer_1 = &pyramid_base;
            let layer_2 = layer_1.get_next_layer().unwrap();
            let layer_3 = layer_2.get_next_layer().unwrap();
            let layer_4 = layer_3.get_next_layer().unwrap();

            // Test detail_coefficients after processing
            assert_slices_approx_equal(
                layer_4.get_detail_coefficients(),
                &layer4::NEW_DETAIL_COEFFICIENTS,
            );
            assert_slices_approx_equal(
                layer_3.get_detail_coefficients(),
                &layer3::NEW_DETAIL_COEFFICIENTS,
            );
            assert_slices_approx_equal(
                layer_2.get_detail_coefficients(),
                &layer2::NEW_DETAIL_COEFFICIENTS,
            );
            assert_slices_approx_equal(
                layer_1.get_detail_coefficients(),
                &layer1::NEW_DETAIL_COEFFICIENTS,
            );
        }

        pyramid_base.rebuild(&upsample_smoothing);
        {
            let layer_1 = &pyramid_base;
            let layer_2 = layer_1.get_next_layer().unwrap();
            let layer_3 = layer_2.get_next_layer().unwrap();
            let layer_4 = layer_3.get_next_layer().unwrap();

            // Test rebuilt
            assert_slices_approx_equal(layer_4.get_refined(), &layer4::REBUILT);
            assert_slices_approx_equal(layer_3.get_refined(), &layer3::REBUILT);
            assert_slices_approx_equal(layer_2.get_refined(), &layer2::REBUILT);
            assert_slices_approx_equal(layer_1.get_refined(), &layer1::REBUILT);
        }
    }
}
