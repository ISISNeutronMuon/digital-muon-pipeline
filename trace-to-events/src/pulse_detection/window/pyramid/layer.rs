use super::Real;
use crate::{
    channels::LayerProcessingSettings,
    pulse_detection::window::{
        convolution_filter::ConvolutionFilter,
        pyramid::{ConvolutionCache, DetailCoefficients, downsample, upsample},
    },
};

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
        subdivide_padding: usize,
        refined_padding: usize,
        next: Box<Layer>,
    ) -> Self {
        Self {
            settings,
            subdivided: ConvolutionCache::new(subdivide_padding),
            refined: ConvolutionCache::new(refined_padding),
            detail_coefficients: DetailCoefficients::new(),
            rebuilt: ConvolutionCache::new(refined_padding),
            next,
        }
    }

    fn init_size(&mut self, size: usize) {
        self.subdivided.init_size(size >> 1);
        self.refined.init_size(size);
        self.detail_coefficients.init_size(size);
        self.rebuilt.init_size(size);
        if let Layer::Level(next_level) = self.next.as_mut() {
            next_level.init_size(size >> 1);
        }
    }

    pub(super) fn process(
        &mut self,
        source: &[Real],
        refinement_smoothing: &ConvolutionFilter,
        subdivide_smoothing: &ConvolutionFilter,
    ) {
        // Downsample from source.
        let padding = self.subdivided.get_padding();
        downsample(source, &mut self.subdivided, padding);
        self.subdivided.convolve(subdivide_smoothing);

        // Upsample from the next layer's subdivided.
        let padding = self.refined.get_padding();
        upsample(&self.subdivided, &mut self.refined, padding);
        self.refined.convolve(refinement_smoothing);

        // Extract detail coefficients.
        for (coef, (src, rfn)) in self
            .detail_coefficients
            .iter_mut()
            .zip(Iterator::zip(source.iter(), self.refined.convolved.iter()))
        {
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
        self.next
            .process(&self.subdivided, refinement_smoothing, subdivide_smoothing);
    }

    pub(super) fn rebuild(&mut self, refinement_smoothing: &ConvolutionFilter) {
        if let Layer::Level(layer_level) = self.next.as_mut() {
            // Propagate rebuild
            layer_level.rebuild(refinement_smoothing);

            let padding = self.rebuilt.get_padding();
            upsample(&layer_level.rebuilt, &mut self.rebuilt, padding);
            self.rebuilt.convolve(refinement_smoothing);

            // Rebuilt is the sum of the next layer's `rebuilt` (upsampled and convolved), and the current detail coefficietns.
            // Note that if output is Some, we use this in place of `rebuilt`.
            for (coef, det) in self
                .rebuilt
                .convolved
                .iter_mut()
                .zip(self.detail_coefficients.0.iter())
            {
                *coef += *det
            }
        } else {
            // Apex case (rebuilt case is the sum of refined and detail_coefficient).
            for (coef, (rfn, det)) in self.rebuilt.convolved.iter_mut().zip(Iterator::zip(
                self.refined.convolved.iter(),
                self.detail_coefficients.0.iter(),
            )) {
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
        mut layer_settings: Vec<LayerProcessingSettings>,
        subdivide_padding: usize,
        refined_padding: usize,
    ) -> Self {
        layer_settings
            .pop()
            .map(|settings| {
                let next = Box::new(Layer::new(
                    layer_settings,
                    subdivide_padding,
                    refined_padding,
                ));
                Self::Level(LayerLevel::new(
                    settings,
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
        refinement_smoothing: &ConvolutionFilter,
        subdivide_smoothing: &ConvolutionFilter,
    ) {
        //  Propagate recursive method
        match self {
            Layer::Level(layer_level) => {
                layer_level.process(source, refinement_smoothing, subdivide_smoothing)
            }
            Layer::Apex => (),
        }
    }

    pub(super) fn init_size(&mut self, size: usize) {
        if let Layer::Level(layer_level) = self {
            layer_level.init_size(size);
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
        convolution_filter::KernelType
    };
    use num::Integer;

    fn assert_layer_settings_default(settings: &LayerProcessingSettings) {
        assert!(settings.denoise_threshold.is_none());
        assert!(settings.enhance_threshold_factor.is_none());
        assert!(settings.multiply_factor.is_none());
    }

    fn assert_layer_sizes(layer_level: &LayerLevel, size: usize, padding: usize) {
        assert_eq!(layer_level.detail_coefficients.len(), size);
        assert_eq!(layer_level.rebuilt.len(), size);
    }

    #[test]
    fn test_two_layers() {
        let settings = vec![LayerProcessingSettings::default()];
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));

        let mut base = Layer::new(settings, gamma.kernel_size() / 2, alpha.kernel_size() / 2);
        base.init_size(SIZE);
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

        let mut base = Layer::new(settings, gamma.kernel_size() / 2, alpha.kernel_size() / 2);
        base.init_size(SIZE);
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

    fn assert_iters_equal<'a>(output: impl ExactSizeIterator<Item = &'a Real>, expected_data: impl ExactSizeIterator<Item = &'a Real>) {
        assert_eq!(output.len(), expected_data.len());

        for (out, exp) in Iterator::zip(output, expected_data) {
            assert_eq!(out, exp);
        }
    }

    #[test]
    fn test_layer_level_unconvolved() {
        let mut layer_level = LayerLevel::new(
            LayerProcessingSettings::default(),
            0,
            0,
            Box::new(Layer::Apex),
        );
        layer_level.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer_level.process(DATA.as_slice(), &alpha, &gamma);

        let output = layer_level.subdivided.as_ref().into_iter();
        let expected_data = DATA.iter().step_by(2);
        assert_iters_equal(output, expected_data.into_iter());

        let output = layer_level.refined.as_ref().into_iter();
        let expected_data = DATA
            .iter()
            .enumerate()
            .map(|(i, v)| if i.is_even() { v } else { &0.0 });
        assert_iters_equal(output, expected_data);
    }

    #[test]
    fn test_layer_unconvolved() {
        let mut layer = Layer::new(vec![LayerProcessingSettings::default()], 0, 0);
        layer.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.process(DATA.as_slice(), &alpha, &gamma);

        match layer {
            Layer::Apex => unreachable!(),
            Layer::Level(layer_level) => {
                let output = layer_level.subdivided.as_ref().into_iter();
                let expected_data = DATA.iter().step_by(2);
                assert_iters_equal(output, expected_data);
                
                let output = layer_level.refined.as_ref().into_iter();
                let expected_data = DATA
                    .iter()
                    .enumerate()
                    .map(|(i, v)| if i.is_even() { v } else { &0.0 });
                assert_iters_equal(output, expected_data);

                assert!(matches!(*layer_level.next, Layer::Apex));
            }
        }
    }

    #[test]
    fn test_two_layer_unconvolved_process() {
        let mut layer = Layer::new(
            vec![
                LayerProcessingSettings::default(),
                LayerProcessingSettings::default(),
            ],
            0,
            0,
        );
        layer.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.process(DATA.as_slice(), &alpha, &gamma);

        match layer {
            Layer::Apex => unreachable!(),
            Layer::Level(layer_level) => {
                let output = layer_level.subdivided.as_ref().into_iter();
                let expected_data = DATA.iter().step_by(2);
                assert_iters_equal(output, expected_data);

                let output = layer_level.refined.as_ref().into_iter();
                let expected_data = DATA
                    .iter()
                    .enumerate()
                    .map(|(i, v)| if i.is_even() { v } else { &0.0 });
                assert_iters_equal(output, expected_data);

                match *layer_level.next {
                    Layer::Apex => unreachable!(),
                    Layer::Level(layer_level) => {
                        let output = layer_level.subdivided.as_ref().into_iter();
                        let expected_data = DATA.iter().step_by(4);
                        assert_iters_equal(output, expected_data);

                        let output = layer_level.refined.as_ref().into_iter();
                        let expected_data = DATA
                            .iter()
                            .step_by(2)
                            .enumerate()
                            .map(|(i, v)| if i.is_even() { v } else { &0.0 });
                        assert_iters_equal(output, expected_data);

                        assert!(matches!(*layer_level.next, Layer::Apex));
                    }
                }
            }
        }
    }

    #[test]
    fn test_two_layer_unconvolved_rebuild() {
        let mut layer = Layer::new(
            vec![
                LayerProcessingSettings::default(),
                LayerProcessingSettings::default(),
            ],
            0,
            0,
        );
        layer.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.process(DATA.as_slice(), &alpha, &gamma);
        layer.rebuild(&alpha);

        match layer {
            Layer::Apex => unreachable!(),
            Layer::Level(layer_level) => {
                let output = layer_level.rebuilt.as_ref().into_iter();
                let expected_data = DATA.iter();
                assert_iters_equal(output, expected_data);
                
                match *layer_level.next {
                    Layer::Apex => unreachable!(),
                    Layer::Level(layer_level) => {
                        let output = layer_level.rebuilt.as_ref().into_iter();
                        let expected_data = DATA.iter().step_by(2);
                        assert_iters_equal(output, expected_data);

                        assert!(matches!(*layer_level.next, Layer::Apex));
                    }
                }
            }
        }
    }
}
