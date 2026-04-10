use super::Real;
use crate::{
    channels::LayerProcessingSettings,
    pulse_detection::window::{
        convolution_filter::ConvolutionFilter,
        pyramid::{ConvolutionCache, DetailCoefficients, downsample, upsample},
    },
};

/// Non-terminating struct of the `Layer` enum, containtaining the next `Layer` in the sequence.
#[derive(Default, Clone)]
pub(super) struct LayerLevel {
    settings: LayerProcessingSettings,
    subdivided: ConvolutionCache,
    refined: ConvolutionCache,
    detail_coefficients: DetailCoefficients,
    rebuilt: ConvolutionCache,
    next_layer: Option<Box<LayerLevel>>,
}

impl LayerLevel {
    pub(super) fn new(
        settings: LayerProcessingSettings,
        subdivide_padding: usize,
        refined_padding: usize,
        mut next_settings_tail: Vec<LayerProcessingSettings>,
    ) -> Self {
        let next_settings = next_settings_tail.pop();
        let next_layer = next_settings.map(|layer_settings|
            Box::new(LayerLevel::new(
                layer_settings,
                subdivide_padding,
                refined_padding,
                next_settings_tail
            ))
        );
        Self {
            settings,
            subdivided: ConvolutionCache::new(subdivide_padding),
            refined: ConvolutionCache::new(refined_padding),
            detail_coefficients: DetailCoefficients::new(),
            rebuilt: ConvolutionCache::new(refined_padding),
            next_layer,
        }
    }

    pub(super) fn init_size(&mut self, size: usize) {
        self.subdivided.init_size(size >> 1);
        self.refined.init_size(size);
        self.detail_coefficients.init_size(size);
        self.rebuilt.init_size(size);
        if let Some(next_layer) = self.next_layer.as_mut() {
            next_layer.init_size(size >> 1);
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
        self.detail_coefficients.extract_from_slices(source, &self.refined);

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
        if let Some(next_layer) = &mut self.next_layer {
            next_layer.process(&self.subdivided, refinement_smoothing, subdivide_smoothing);
        }
    }

    pub(super) fn rebuild(&mut self, refinement_smoothing: &ConvolutionFilter) -> &[Real] {
        if let Some(next_layer) = &mut self.next_layer {
            // Propagate rebuild
            next_layer.rebuild(refinement_smoothing);

            // Rebuilt is the sum of the next layer's `rebuilt` (upsampled and convolved), and the current detail_coefficients.
            let padding = self.rebuilt.get_padding();
            upsample(&next_layer.rebuilt, &mut self.rebuilt, padding);
            self.rebuilt.convolve(refinement_smoothing);
            self.rebuilt.append_slice(&self.detail_coefficients);
        } else {
            // Apex case (rebuilt case is the sum of refined and detail_coefficient).
            self.rebuilt.sum_from_slices(&self.refined, &self.detail_coefficients);
        }
        &self.rebuilt
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
    pub(super) fn process(
        &mut self,
        source: &[Real],
        refinement_smoothing: &ConvolutionFilter,
        subdivide_smoothing: &ConvolutionFilter,
    ) {
        //  Propagate recursive method
        if let Layer::Level(layer_level) = self {
            layer_level.process(source, refinement_smoothing, subdivide_smoothing)
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

    fn assert_layer_sizes(layer_level: &LayerLevel, size: usize, _padding: usize) {
        assert_eq!(layer_level.detail_coefficients.len(), size);
        assert_eq!(layer_level.rebuilt.len(), size);
    }

    #[test]
    fn test_two_layers() {
        let settings = LayerProcessingSettings::default();
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));

        let mut base = LayerLevel::new(settings, gamma.kernel_size() / 2, alpha.kernel_size() / 2, Default::default());
        assert_layer_settings_default(&base.settings);
        assert_layer_sizes(&base, 0, 0);
        assert!(base.next_layer.is_none());

        base.init_size(SIZE);
        assert_layer_settings_default(&base.settings);
        assert_layer_sizes(&base, SIZE, 2);
        assert!(base.next_layer.is_none());
    }

    #[test]
    fn test_three_layers() {
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));

        let mut base = LayerLevel::new(LayerProcessingSettings::default(), gamma.kernel_size() / 2, alpha.kernel_size() / 2, vec![LayerProcessingSettings::default()]);
        base.init_size(SIZE);
        assert_layer_settings_default(&base.settings);
        assert_layer_sizes(&base, SIZE, 2);
        assert!(base.next_layer.is_some());
        match base.next_layer.as_ref() {
            Some(layer_level) => {
                assert_layer_settings_default(&layer_level.settings);
                assert_layer_sizes(&layer_level, SIZE >> 1, 2);
                assert!(layer_level.next_layer.is_none());
            }
            None => unreachable!(),
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
            Default::default(),
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
        let mut layer = LayerLevel::new(LayerProcessingSettings::default(), 0, 0, vec![]);
        layer.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.process(DATA.as_slice(), &alpha, &gamma);

        let output = layer.subdivided.as_ref().into_iter();
        let expected_data = DATA.iter().step_by(2);
        assert_iters_equal(output, expected_data);
        
        let output = layer.refined.as_ref().into_iter();
        let expected_data = DATA
            .iter()
            .enumerate()
            .map(|(i, v)| if i.is_even() { v } else { &0.0 });
        assert_iters_equal(output, expected_data);

        assert!(layer.next_layer.is_none());
    }

    #[test]
    fn test_two_layer_unconvolved_process() {
        let mut layer = LayerLevel::new(
            LayerProcessingSettings::default(),
            0,
            0,
            vec![LayerProcessingSettings::default()]
        );
        layer.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.process(DATA.as_slice(), &alpha, &gamma);

        let output = layer.subdivided.as_ref().into_iter();
        let expected_data = DATA.iter().step_by(2);
        assert_iters_equal(output, expected_data);

        let output = layer.refined.as_ref().into_iter();
        let expected_data = DATA
            .iter()
            .enumerate()
            .map(|(i, v)| if i.is_even() { v } else { &0.0 });
        assert_iters_equal(output, expected_data);

        match layer.next_layer {
            None => unreachable!(),
            Some(layer_level) => {
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

                assert!(layer_level.next_layer.is_none());
            }
        }
    }

    #[test]
    fn test_two_layer_unconvolved_rebuild() {
        let mut layer = LayerLevel::new(
            LayerProcessingSettings::default(),
            0,
            0,
            vec![LayerProcessingSettings::default()],
        );
        layer.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.process(DATA.as_slice(), &alpha, &gamma);
        layer.rebuild(&alpha);

        let output = layer.rebuilt.as_ref().into_iter();
        let expected_data = DATA.iter();
        assert_iters_equal(output, expected_data);
        
        match layer.next_layer {
            None => unreachable!(),
            Some(layer) => {
                let output = layer.rebuilt.as_ref().into_iter();
                let expected_data = DATA.iter().step_by(2);
                assert_iters_equal(output, expected_data);

                assert!(matches!(layer.next_layer, None));
            }
        }
    }
}
