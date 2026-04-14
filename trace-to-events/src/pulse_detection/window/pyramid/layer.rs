use super::Real;
use crate::{
    channels::LayerProcessingSettings,
    pulse_detection::window::{
        convolution_filter::ConvolutionFilter,
        pyramid::{ConvolutionCache, DetailCoefficients},
    },
};

/// Linked list element the [PyramidFilter] structure.
///
/// The `size` of the layer, is taken to mean the size [Self::refined] and [Self::detail_coefficients].
/// [Self::subdived] is has size half of the layer's size.
#[derive(Default, Clone)]
pub(super) struct Layer {
    /// Cache to which an input is downsampled.
    subdivided: ConvolutionCache,
    /// Cache to which the
    refined: ConvolutionCache,
    /// Vector from which details are recorded and processed.
    detail_coefficients: DetailCoefficients,
    /// None if this layer is the apex, otherwise a layer of size half of this one.
    next_layer: Option<Box<Layer>>,
}

impl Layer {
    /// Creates a new layer of the pyramid smoothing algorithm.
    ///
    /// # Parameters
    /// - settings:
    /// - subdivide_padding:
    /// - refined_padding:
    /// - next_settings_tail:
    pub(super) fn new(
        settings: LayerProcessingSettings,
        subdivide_padding: usize,
        refined_padding: usize,
        mut next_settings_tail: Vec<LayerProcessingSettings>,
    ) -> Self {
        let next_settings_head = next_settings_tail.pop();
        let next_layer = next_settings_head.map(|next_settings_head| {
            Box::new(Layer::new(
                next_settings_head,
                subdivide_padding,
                refined_padding,
                next_settings_tail,
            ))
        });
        Self {
            subdivided: ConvolutionCache::new(subdivide_padding),
            refined: ConvolutionCache::new(refined_padding),
            detail_coefficients: DetailCoefficients::new(settings),
            next_layer,
        }
    }

    /// Initialises the layer to have the given size, and recursively propagate
    /// a size of half this value to the next layer.
    ///
    /// The size of the layer is defined as the size of [Self::refined] and [Self::detail_coefficients],
    /// whilst [Self::subdivided] is half this size.
    ///
    /// # Parameters
    /// - size: the size from which to initialise the layer's fields.
    pub(super) fn init_size(&mut self, size: usize) {
        self.subdivided.init_size(size >> 1);
        self.refined.init_size(size);
        self.detail_coefficients.init_size(size);
        if let Some(next_layer) = self.next_layer.as_mut() {
            next_layer.init_size(size >> 1);
        }
    }

    /// Performs the first stage of the smoothing algorithm recursively.
    ///
    /// # Parameters
    /// - source: an input vector whose length of equal to the layer's size.
    /// - refinement_smoothing: the smoothing filter to be applied after upsampling.
    /// - subdivide_smoothing: the smoothing filter to be applied after downsampling.
    ///
    /// Given the slice `source` of the appropriate length:
    /// - `source` is downsampled to [Self::subdivided] and then a convolution applied.
    /// - [Self::subdivided] is upsampled to [Self::refined] and then a convolution applied.
    /// - [Self::detail_coefficients] is computed as the difference between `source` and [Self::refined].
    /// - If this layer is not the apex, then [Self::subdivided] is recursively passed as `source` to the next layer.
    pub(super) fn build(
        &mut self,
        source: &[Real],
        refinement_smoothing: &ConvolutionFilter,
        subdivide_smoothing: &ConvolutionFilter,
    ) {
        // Downsample from source.
        self.subdivided.downsample(source);
        self.subdivided.convolve(subdivide_smoothing);

        // Upsample from the next layer's subdivided.
        self.refined.upsample(&self.subdivided);
        self.refined.convolve(refinement_smoothing);

        // Extract detail coefficients.
        self.detail_coefficients
            .extract_from_slices(source, &self.refined);

        //  Recurse method to next layer.
        if let Some(next_layer) = &mut self.next_layer {
            next_layer.build(&self.subdivided, refinement_smoothing, subdivide_smoothing);
        }
    }

    /// Performs the second stage of the smoothing algorithm recursively.
    /// Should be called after [Self::build].
    ///
    /// Calls [DetailCoefficient::process] and propagates the method recursively.
    pub(super) fn process(&mut self) {
        // Process detail coefficients.
        self.detail_coefficients.process();

        //  Recurse method to next layer.
        if let Some(next_layer) = &mut self.next_layer {
            next_layer.process();
        }
    }

    /// Performs the third and last stage of the smoothing algorithm recursively.
    /// Should be called after [Self::process].
    ///
    /// # Parameters
    /// - refinement_smoothing: the smoothing filter to be applied after upsampling.
    ///
    /// - If this layer is not the apex, then:
    ///    - Recursivly propagate the method to the next layer.
    ///    - The result of the recursion is upsampled to [Self::refined] and then a convolution applied.
    /// - [Self::refined] has the values of [Self::detail_coefficients] added to it elementwise.
    /// - A slice to [Self::refined] is returned.
    pub(super) fn rebuild(&mut self, refinement_smoothing: &ConvolutionFilter) -> &[Real] {
        if let Some(next_layer) = &mut self.next_layer {
            // Propagate rebuild
            let next_layer_rebuilt = next_layer.rebuild(refinement_smoothing);

            // Rebuilt is the sum of the next layer's `rebuilt` (upsampled and convolved), and the current detail_coefficients.
            self.refined.upsample(next_layer_rebuilt);
            self.refined.convolve(refinement_smoothing);
        }
        self.refined.inject_details(&self.detail_coefficients);
        &self.refined
    }
}

#[cfg(test)]
impl Layer {
    pub(super) fn get_subdivided(&self) -> &ConvolutionCache {
        &self.subdivided
    }

    pub(super) fn get_refined(&self) -> &ConvolutionCache {
        &self.refined
    }

    pub(super) fn get_detail_coefficients(&self) -> &DetailCoefficients {
        &self.detail_coefficients
    }

    pub(super) fn get_next_layer(&self) -> Option<&Box<Layer>> {
        self.next_layer.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pulse_detection::window::convolution_filter::KernelType, test_data::assert_iters_equal,
    };
    use num::Integer;

    fn assert_layer_sizes(layer_level: &Layer, size: usize, _padding: usize) {
        assert_eq!(layer_level.detail_coefficients.len(), size);
        assert_eq!(layer_level.refined.len(), size);
    }

    #[test]
    fn test_two_layers() {
        let settings = LayerProcessingSettings::default();
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));

        let mut base = Layer::new(
            settings,
            gamma.kernel_size() / 2,
            alpha.kernel_size() / 2,
            Default::default(),
        );
        assert_layer_sizes(&base, 0, 0);
        assert!(base.next_layer.is_none());

        base.init_size(SIZE);
        assert_layer_sizes(&base, SIZE, 2);
        assert!(base.next_layer.is_none());
    }

    #[test]
    fn test_three_layers() {
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));

        let mut base = Layer::new(
            LayerProcessingSettings::default(),
            gamma.kernel_size() / 2,
            alpha.kernel_size() / 2,
            vec![LayerProcessingSettings::default()],
        );
        base.init_size(SIZE);
        assert_layer_sizes(&base, SIZE, 2);
        assert!(base.next_layer.is_some());
        match base.next_layer.as_ref() {
            Some(layer_level) => {
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

    #[test]
    fn test_layer_level_unconvolved() {
        let mut layer_level =
            Layer::new(LayerProcessingSettings::default(), 0, 0, Default::default());
        layer_level.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer_level.build(DATA.as_slice(), &alpha, &gamma);

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
        let mut layer = Layer::new(LayerProcessingSettings::default(), 0, 0, vec![]);
        layer.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.build(DATA.as_slice(), &alpha, &gamma);

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
        let mut layer = Layer::new(
            LayerProcessingSettings::default(),
            0,
            0,
            vec![LayerProcessingSettings::default()],
        );
        layer.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.build(DATA.as_slice(), &alpha, &gamma);

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
        let mut layer = Layer::new(
            LayerProcessingSettings::default(),
            0,
            0,
            vec![LayerProcessingSettings::default()],
        );
        layer.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.build(DATA.as_slice(), &alpha, &gamma);
        layer.rebuild(&alpha);

        let output = layer.refined.as_ref().into_iter();
        let expected_data = DATA.iter();
        assert_iters_equal(output, expected_data);

        match layer.next_layer {
            None => unreachable!(),
            Some(layer) => {
                let output = layer.refined.as_ref().into_iter();
                let expected_data = DATA.iter().step_by(2);
                assert_iters_equal(output, expected_data);

                assert!(matches!(layer.next_layer, None));
            }
        }
    }
}
