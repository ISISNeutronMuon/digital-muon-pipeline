use super::Real;
use crate::pulse_detection::window::{
    convolution_filter::ConvolutionFilter,
    pyramid::{ConvolutionCache, DetailCoefficients, downsample, upsample},
};

#[derive(Default, Clone)]
pub(crate) struct LayerProcessingSettings {
    pub(super) denoise_threshold: Option<Real>,
    pub(super) enhance_threshold_factor: Option<(Real, Real)>,
    pub(super) multiply_factor: Option<Real>,
}

/// Non-terminating struct of the `Layer` enum, containtaining the next `Layer` in the sequence.
#[derive(Clone)]
pub(super) struct LayerLevel {
    settings: LayerProcessingSettings,
    pub(super) subdivided: ConvolutionCache,
    pub(super) refined: ConvolutionCache,
    pub(super) detail_coefficients: DetailCoefficients,
    rebuilt: ConvolutionCache,
    next: Box<Layer>,
}

impl LayerLevel {
    fn new(
        settings: LayerProcessingSettings,
        size: usize,
        subdivide_padding: usize,
        refined_padding: usize,
        next: Box<Layer>,
    ) -> Self {
        Self {
            settings,
            subdivided: ConvolutionCache::new(size >> 1, subdivide_padding),
            refined: ConvolutionCache::new(size, refined_padding),
            detail_coefficients: DetailCoefficients::new(size),
            rebuilt: ConvolutionCache::new(size, refined_padding),
            next,
        }
    }

    pub(super) fn process(
        &mut self,
        source: &[Real],
        alpha: &ConvolutionFilter,
        gamma: &ConvolutionFilter,
    ) {
        // Downsample from source.
        let padding = self.subdivided.padding;
        downsample(source, &mut self.subdivided, padding);
        self.subdivided.convolve(gamma);

        // Upsample from the next layer's subdivided.
        let padding = self.refined.padding;
        upsample(&self.subdivided, &mut self.refined, padding);
        self.refined.convolve(alpha);

        // Extract detail coefficients.
        for (coef, (src, rfn)) in self.detail_coefficients
            .iter_mut()
            .zip(Iterator::zip(source.iter(), self.refined.convolved.iter())) {
                *coef = *src - *rfn;
        }

        // Process detail coefficients.
        if let Some(denoise_threshold) = self.settings.denoise_threshold {
            self.detail_coefficients.denoise(denoise_threshold);
        }
        if let Some((enhance_threshold, enhance_factor)) = self.settings.enhance_threshold_factor {
            self.detail_coefficients
                .enhance(enhance_threshold, enhance_factor);
        }
        if let Some(multiply_factor) = self.settings.multiply_factor {
            self.detail_coefficients.multiply(multiply_factor);
        }

        //  Recurse method to next layer.
        self.next.process(&self.subdivided, alpha, gamma);
    }

    pub(super) fn rebuild(&mut self, alpha: &ConvolutionFilter) {
        if let Layer::Level(layer_level) = self.next.as_mut() {
            // Propagate rebuild
            layer_level.rebuild(alpha);

            let padding = self.rebuilt.padding;
            upsample(&layer_level.rebuilt, &mut self.rebuilt, padding);
            self.rebuilt.convolve(alpha);

            // Rebuilt is the sum of the next layer's `rebuilt` (upsampled and convolved), and the current detail coefficietns.
            // Note that if output is Some, we use this in place of `rebuilt`.
            for (coef, det) in self.rebuilt
                .convolved
                .iter_mut()
                .zip(self.detail_coefficients.0.iter()) {
                *coef += *det
            }

        } else {
            // Apex case (rebuilt case is the sum of refined and detail_coefficient).
            for (coef, (rfn, det)) in self.rebuilt
                .convolved
                .iter_mut()
                .zip(Iterator::zip(self.refined.convolved.iter(), self.detail_coefficients.0.iter())) {
                    *coef = *rfn + *det;
            }
        }
    }
}

/// Linked list implementation of the pyramid. As an invariant each layer of the sequence contains only `ConvolutionCache`s of the same length.
#[derive(Default, Clone)]
pub(super) enum Layer {
    #[default]
    Apex,
    Level(LayerLevel),
}

impl Layer {
    pub(super) fn new(
        size: usize,
        mut layer_settings: Vec<LayerProcessingSettings>,
        subdivide_padding: usize,
        refined_padding: usize,
    ) -> Self {
        layer_settings
            .pop()
            .map(|settings| {
                let next = Box::new(Layer::new(
                    size >> 1,
                    layer_settings,
                    subdivide_padding,
                    refined_padding,
                ));
                Self::Level(LayerLevel::new(
                    settings,
                    size,
                    subdivide_padding,
                    refined_padding,
                    next,
                ))
            })
            .unwrap_or_default()
    }

    pub(super) fn process(
        &mut self,
        source: &[Real],
        alpha: &ConvolutionFilter,
        gamma: &ConvolutionFilter,
    ) {
        //  Propagate recursive method
        match self {
            Layer::Level(layer_level) => layer_level.process(source, alpha, gamma),
            Layer::Apex => (),
        }
    }

    pub(super) fn rebuild(&mut self, alpha: &ConvolutionFilter) -> Option<&[Real]> {
        if let Layer::Level(layer_level) = self {
            layer_level.rebuild(alpha);
            Some(&layer_level.rebuilt)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse_detection::window::{
        SliceWindow, convolution_filter::KernelType, fft_inverse::FftInverse,
    };
    use num::Integer;
    use rustfft::num_complex::{Complex, ComplexFloat};

    fn assert_layer_settings_default(settings: &LayerProcessingSettings) {
        assert!(settings.denoise_threshold.is_none());
        assert!(settings.enhance_threshold_factor.is_none());
        assert!(settings.multiply_factor.is_none());
    }

    fn assert_convolution_cache_sizes(cache: &ConvolutionCache, size: usize, padding: usize) {
        assert_eq!(cache.raw.len(), size + padding);
        assert_eq!(cache.convolved.len(), size);
    }

    fn assert_layer_sizes(layer_level: &LayerLevel, size: usize, padding: usize) {
        assert_convolution_cache_sizes(&layer_level.subdivided, size, padding);
        assert_convolution_cache_sizes(&layer_level.refined, size, padding);
        assert_eq!(layer_level.detail_coefficients.len(), size);
        assert_eq!(layer_level.rebuilt.len(), size);
    }

    #[test]
    fn test_two_layers() {
        let settings = vec![LayerProcessingSettings::default()];
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));

        let base = Layer::new(
            SIZE,
            settings,
            gamma.kernel_size() / 2,
            alpha.kernel_size() / 2,
        );
        assert!(matches!(base, Layer::Level(_)));
        match base {
            Layer::Level(layer_level) => {
                assert_layer_settings_default(&layer_level.settings);
                assert_layer_sizes(&layer_level, SIZE, 2);
                assert!(matches!(layer_level.next.as_ref(), Layer::Apex));
            }
            Layer::Apex => unreachable!(),
        }
    }

    #[test]
    fn test_three_layers() {
        let settings = vec![
            LayerProcessingSettings::default(),
            LayerProcessingSettings::default(),
        ];
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));

        let base = Layer::new(
            SIZE,
            settings,
            gamma.kernel_size() / 2,
            alpha.kernel_size() / 2,
        );
        assert!(matches!(base, Layer::Level(_)));
        match base {
            Layer::Level(layer_level) => {
                assert_layer_settings_default(&layer_level.settings);
                assert_layer_sizes(&layer_level, SIZE, 2);
                assert!(matches!(layer_level.next.as_ref(), Layer::Level(_)));
                match layer_level.next.as_ref() {
                    Layer::Level(layer_level) => {
                        assert_layer_settings_default(&layer_level.settings);
                        assert_layer_sizes(&layer_level, SIZE >> 1, 2);
                        assert!(matches!(layer_level.next.as_ref(), Layer::Apex));
                    }
                    Layer::Apex => unreachable!(),
                }
            }
            Layer::Apex => unreachable!(),
        }
    }

    const SIZE: usize = 36;
    const DATA: [Real; SIZE] = [
        0.0, 1.0, 0.0, 2.0, 1.0, 3.0, 5.0, 4.0, 3.2, 1.1, 0.1, 0.0, 0.0, 1.0, 8.0, 2.0, 1.0, 3.0,
        5.0, 4.0, 3.2, 1.1, 9.1, 4.0, 2.1, 1.5, 0.0, 2.0, 1.0, 3.0, 5.0, 4.0, 3.2, 1.1, 3.1, 2.0,
    ];

    #[test]
    fn test_layer_level_unconvolved() {
        let mut layer_level = LayerLevel::new(
            LayerProcessingSettings::default(),
            SIZE,
            0,
            0,
            Box::new(Layer::Apex),
        );
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer_level.process(DATA.as_slice(), &alpha, &gamma);

        let output = layer_level.subdivided.as_ref().into_iter();
        let expected_data = DATA.iter().step_by(2);
        assert_eq!(output.len(), expected_data.len());

        for (out, exp) in Iterator::zip(output, expected_data) {
            assert_eq!(out, exp);
        }

        let output = layer_level.refined.as_ref().into_iter().cloned();
        let expected_data = DATA
            .iter()
            .enumerate()
            .map(|(i, v)| if i.is_even() { *v } else { 0.0 });
        assert_eq!(output.len(), expected_data.len());

        for (out, exp) in Iterator::zip(output, expected_data) {
            assert_eq!(out, exp);
        }
    }

    #[test]
    fn test_layer_unconvolved() {
        let mut layer = Layer::new(SIZE, vec![LayerProcessingSettings::default()], 0, 0);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.process(DATA.as_slice(), &alpha, &gamma);

        match layer {
            Layer::Apex => unreachable!(),
            Layer::Level(layer_level) => {
                let output = layer_level.subdivided.as_ref().into_iter();
                let expected_data = DATA.iter().step_by(2);
                assert_eq!(output.len(), expected_data.len());

                for (out, exp) in Iterator::zip(output, expected_data) {
                    assert_eq!(out, exp);
                }

                let output = layer_level.refined.as_ref().into_iter().cloned();
                let expected_data = DATA
                    .iter()
                    .enumerate()
                    .map(|(i, v)| if i.is_even() { *v } else { 0.0 });
                assert_eq!(output.len(), expected_data.len());

                for (out, exp) in Iterator::zip(output, expected_data) {
                    assert_eq!(out, exp);
                }

                assert!(matches!(*layer_level.next, Layer::Apex));
            }
        }
    }

    #[test]
    fn test_two_layer_unconvolved() {
        let mut layer = Layer::new(
            SIZE,
            vec![
                LayerProcessingSettings::default(),
                LayerProcessingSettings::default(),
            ],
            0,
            0,
        );
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.process(DATA.as_slice(), &alpha, &gamma);

        match layer {
            Layer::Apex => unreachable!(),
            Layer::Level(layer_level) => {
                let output = layer_level.subdivided.as_ref().into_iter();
                let expected_data = DATA.iter().step_by(2);
                assert_eq!(output.len(), expected_data.len());

                for (out, exp) in Iterator::zip(output, expected_data) {
                    assert_eq!(out, exp);
                }

                let output = layer_level.refined.as_ref().into_iter().cloned();
                let expected_data = DATA
                    .iter()
                    .enumerate()
                    .map(|(i, v)| if i.is_even() { *v } else { 0.0 });
                assert_eq!(output.len(), expected_data.len());

                for (out, exp) in Iterator::zip(output, expected_data) {
                    assert_eq!(out, exp);
                }

                match *layer_level.next {
                    Layer::Apex => unreachable!(),
                    Layer::Level(layer_level) => {
                        let output = layer_level.subdivided.as_ref().into_iter();
                        let expected_data = DATA.iter().step_by(4);
                        assert_eq!(output.len(), expected_data.len());

                        for (out, exp) in Iterator::zip(output, expected_data) {
                            assert_eq!(out, exp);
                        }

                        let output = layer_level.refined.as_ref().into_iter().cloned();
                        let expected_data = DATA
                            .iter()
                            .step_by(2)
                            .enumerate()
                            .map(|(i, v)| if i.is_even() { *v } else { 0.0 });
                        assert_eq!(output.len(), expected_data.len());

                        for (out, exp) in Iterator::zip(output, expected_data) {
                            assert_eq!(out, exp);
                        }
                        assert!(matches!(*layer_level.next, Layer::Apex));
                    }
                }
            }
        }
    }

    fn reverse(alpha: &[Real], support: &[i32]) -> Vec<Real> {
        let mut gamma = vec![0.0; 5];
        let fft = FftInverse::new(200, 6, support.to_vec(), Complex::recip);
        fft.apply_to_slice(alpha, gamma.as_mut_slice());
        gamma
    }

    #[test]
    fn test_reverse() {
        let settings = vec![LayerProcessingSettings {
            denoise_threshold: Some(1.1),
            enhance_threshold_factor: Some((1.1, 1.3)),
            multiply_factor: Some(1.1),
        }];
        let alpha = vec![0.125, 0.5, 0.75, 0.5, 0.125];
        let gamma = reverse(&alpha, &[-2, -1, 0, 1, 2]);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(alpha));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(gamma));

        let mut base = Layer::new(
            SIZE,
            settings,
            gamma.kernel_size() / 2,
            alpha.kernel_size() / 2,
        );

        base.process(&DATA, &alpha, &gamma);
        let output = base.rebuild(&alpha);
        println!("{output:?}");
    }
}
