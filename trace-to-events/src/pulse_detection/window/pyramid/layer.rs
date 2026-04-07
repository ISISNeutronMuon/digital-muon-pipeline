use super::Real;
use crate::pulse_detection::window::{
    convolution_filter::ConvolutionFilter, pyramid::{ConvolutionCache, DetailCoefficients, downsample, upsample}
};

#[derive(Default, Clone)]
pub(super) struct LayerProcessingSettings {
    pub(super) denoise_threshold: Option<Real>,
    pub(super) enhance_threshold_factor: Option<(Real, Real)>,
    pub(super) multiply_factor: Option<Real>,
}


/// Non-terminating struct of the `Layer` enum, containtaining the next `Layer` in the sequence.
#[derive(Clone)]
pub(super) struct LayerLevel {
    settings: LayerProcessingSettings,
    subdivided: ConvolutionCache,
    refined: ConvolutionCache,
    detail_coefficients: DetailCoefficients,
    rebuilt: ConvolutionCache,
    next: Box<Layer>,
}

impl LayerLevel {
    fn new(
        settings: LayerProcessingSettings,
        size: usize,
        subdivide_padding: usize,
        refined_padding: usize,
        next: Box<Layer>
    ) -> Self {
        Self {
                settings,
                subdivided: ConvolutionCache::new(size, subdivide_padding),
                refined: ConvolutionCache::new(size, refined_padding),
                detail_coefficients: DetailCoefficients::new(size),
                rebuilt: ConvolutionCache::new(size, refined_padding),
                next
        }
    }

    pub(super) fn process(
        &mut self,
        source: &[Real],
        alpha: &ConvolutionFilter,
        gamma: &ConvolutionFilter,
    ) {
        // Downsample from source
        let padding = self.subdivided.padding;
        downsample(source, &mut self.subdivided, padding);
        self.subdivided.convolve(gamma);

        //  Propagate recursive method
        self.next
            .process(&self.subdivided, alpha, gamma);

        // Upsample from next layer
        let padding = self.refined.padding;
        upsample(self.next.get_subdivided(), &mut self.refined, padding);
        self.refined.convolve(alpha);

        // Extract detail coefficients
        self.detail_coefficients
            .iter_mut()
            .enumerate()
            .for_each(|(i, coef)| *coef = source[i] - self.refined[i]);

        // Process detail coefficients
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
    }

    pub(super) fn rebuild(&mut self, alpha: &ConvolutionFilter, output: Option<&mut [Real]>) {
        if let Layer::Level(layer_level) = self.next.as_mut() {
            // Propagate rebuild
            layer_level.rebuild(alpha, None);

            let padding = self.rebuilt.padding;
            upsample(&layer_level.rebuilt, &mut self.rebuilt, padding);
            self.rebuilt.convolve(alpha);

            // Rebuilt is the sum of the next layer's `rebuilt` (upsampled and convolved), and the current detail coefficietns.
            // Note that if output is Some, we use this in place of `rebuilt`.
            output.map(<[Real]>::iter_mut)
                .unwrap_or(self.rebuilt.convolved.iter_mut())
                .enumerate()
                .for_each(|(i, coef)| *coef += self.detail_coefficients[i]);   
        } else {
            // Apex case (rebuilt case is the sum of refined and detail_coefficient).
            self.rebuilt.convolved
                .iter_mut()
                .enumerate()
                .for_each(|(i, coef)| *coef = self.refined[i] + self.detail_coefficients[i]);
        }
    }
}

/// Linked list implementation of the pyramid. As an invariant each layer of the sequence contains only `ConvolutionCache`s of the same length.
#[derive(Clone)]
pub(super) enum Layer {
    Level(LayerLevel),
    Apex(ConvolutionCache),
}

impl Default for Layer {
    fn default() -> Self {
        Self::Apex(Default::default())
    }
}

impl Layer {
    pub(super) fn new(
        size: usize,
        mut layer_settings: Vec<LayerProcessingSettings>,
        subdivide_padding: usize,
        refined_padding: usize,
    ) -> Self {
        if layer_settings.is_empty() {
            Self::Apex(ConvolutionCache::new(size, subdivide_padding))
        } else {
            let settings = layer_settings
                .pop()
                .expect("Vector should be nonempty, this should never fail.");
            let next = Box::new(Layer::new(
                size >> 1,
                layer_settings,
                subdivide_padding,
                refined_padding,
            ));
            Self::Level(LayerLevel::new(settings, size, subdivide_padding, refined_padding, next))
        }
    }

    fn get_subdivided(&self) -> &[Real] {
        match self {
            Layer::Level(layer_level) => &layer_level.subdivided,
            Layer::Apex(convolution_cache) => convolution_cache,
        }
    }

    pub(super) fn process(
        &mut self,
        source: &[Real],
        alpha: &ConvolutionFilter,
        gamma: &ConvolutionFilter,
    ) {
        //  Propagate recursive method
        match self {
            Layer::Level(layer_level) => {
                layer_level.process(source, alpha, gamma);
            }
            Layer::Apex(subdivided) => {
                let padding = subdivided.padding;
                downsample(source, subdivided, padding);
                subdivided.convolve(gamma);
            },
        }
    }

    pub(super) fn rebuild(&mut self, alpha: &ConvolutionFilter, output: Option<&mut [Real]>) {
        if let Layer::Level(layer_level) = self {
            layer_level.rebuild(alpha, output);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustfft::num_complex::{Complex, ComplexFloat};
    use crate::pulse_detection::window::{SliceWindow, convolution_filter::KernelType, fft_inverse::FftInverse};

    const SIZE: usize = 36;
    const DATA : [Real; SIZE] = [0.0,1.0,0.0,2.0,1.0,3.0,5.0,4.0,3.2,1.1,0.1,0.0,0.0,1.0,8.0,2.0,1.0,3.0,5.0,4.0,3.2,1.1,9.1,4.0,2.1,1.5,0.0,2.0,1.0,3.0,5.0,4.0,3.2,1.1,3.1,2.0];

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
        let settings = vec![
            LayerProcessingSettings::default()
        ];
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0,0.0,0.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0,0.0,0.0]));

        let base = Layer::new(SIZE, settings, gamma.kernel_size()/2, alpha.kernel_size()/2);
        assert!(matches!(base, Layer::Level(_)));
        match base {
            Layer::Level(layer_level) => {
                assert_layer_settings_default(&layer_level.settings);
                assert_layer_sizes(&layer_level, SIZE, 2);
                assert!(matches!(layer_level.next.as_ref(), Layer::Apex(_)));
                match layer_level.next.as_ref() {
                    Layer::Level(_) => unreachable!(),
                    Layer::Apex(subdivided) => {
                        assert_convolution_cache_sizes(subdivided, SIZE >> 1, 2);
                    },
                }
            },
            Layer::Apex(_) => unreachable!(),
        }
    }

    #[test]
    fn test_three_layers() {
        let settings = vec![
            LayerProcessingSettings::default(),
            LayerProcessingSettings::default(),
        ];
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0,0.0,0.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0,0.0,0.0]));

        let base = Layer::new(SIZE, settings, gamma.kernel_size()/2, alpha.kernel_size()/2);
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
                        assert!(matches!(layer_level.next.as_ref(), Layer::Apex(_)));
                        match layer_level.next.as_ref() {
                            Layer::Level(_) => unreachable!(),
                            Layer::Apex(subdivided) => {
                                assert_convolution_cache_sizes(subdivided, SIZE >> 2, 2);
                            },
                        }
                    },
                    Layer::Apex(_) => unreachable!(),
                }
            },
            Layer::Apex(_) => unreachable!(),
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
        let settings = vec![
            LayerProcessingSettings { denoise_threshold: Some(1.1), enhance_threshold_factor: Some((1.1, 1.3)), multiply_factor: Some(1.1) }
        ];
        let alpha = vec![0.125, 0.5, 0.75, 0.5, 0.125];
        let gamma = reverse(&alpha, &[-2, -1, 0, 1, 2]);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(alpha));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(gamma));

        let mut base = Layer::new(SIZE, settings, gamma.kernel_size()/2, alpha.kernel_size()/2);

        base.process(&DATA, &alpha, &gamma);
        let mut output = vec![0.0; SIZE];
        base.rebuild(&alpha, Some(&mut output));
        println!("{output:?}");
    }
}