//! A layer in the pyramid smoothing algorithm performs the rescaling, smoothing,
//! and detail extraction at a specific resolution, as well as linking to the next layer.
use crate::{
    channels::LayerProcessingSettings,
    pulse_detection::{
        Real,
        window::{
            convolution_filter::ConvolutionFilter,
            pyramid::traces::{ConvolutionCache, DetailCoefficients},
        },
    },
};

/// Linked list element the [PyramidFilter] structure.
///
/// The `size` of the layer, is taken to mean the size [Self::refined] and [Self::detail_coefficients].
/// [Self::subdived] is has size half of the layer's size.
#[derive(Default, Clone)]
pub(crate) struct PyramidLayer {
    /// Cache to which an input is downsampled.
    subdivided: ConvolutionCache,
    /// Cache to which the downsampled trace is upsampled, and where the processed details are rebuilt.
    refined: ConvolutionCache,
    /// Vector from which details are recorded and processed.
    detail_coefficients: DetailCoefficients,
    /// None if this layer is the apex, otherwise a layer of size half of this one.
    next_layer: Option<Box<PyramidLayer>>,
}

impl PyramidLayer {
    /// Creates a new layer of the pyramid smoothing algorithm and all subsequent layers.
    ///
    /// # Parameters
    /// - layer_settings: vector of [LayerProcessingSettings] in descending order, i.e. starting with the apex.
    /// - downsample_padding: the amount of extra space to include for downsampling.
    /// - upsample_padding: the amount of extra space to include for upsampling.
    pub(crate) fn new(
        mut layer_settings: Vec<LayerProcessingSettings>,
        downsample_padding: usize,
        upsample_padding: usize,
    ) -> Option<Box<Self>> {
        let this_settings = layer_settings.pop();
        this_settings.map(|this_settings| {
            Box::new(Self {
                subdivided: ConvolutionCache::new(downsample_padding),
                refined: ConvolutionCache::new(upsample_padding),
                detail_coefficients: DetailCoefficients::new(this_settings),
                next_layer: PyramidLayer::new(layer_settings, downsample_padding, upsample_padding),
            })
        })
    }

    /// Initialises the layer to have the given size, and recursively propagate
    /// a size of half this value to the next layer.
    ///
    /// The size of the layer is defined as the size of [Self::refined] and [Self::detail_coefficients],
    /// whilst [Self::subdivided] is half this size.
    ///
    /// # Parameters
    /// - size: the size from which to initialise the layer's fields.
    pub(crate) fn init_size(&mut self, size: usize) {
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
    /// - downsample_smoothing: the smoothing filter to be applied after downsampling.
    /// - upsample_smoothing: the smoothing filter to be applied after upsampling.
    ///
    /// Given the slice `source` of the appropriate length:
    /// - `source` is downsampled to [Self::subdivided] and then a convolution applied.
    /// - [Self::subdivided] is upsampled to [Self::refined] and then a convolution applied.
    /// - [Self::detail_coefficients] is computed as the difference between `source` and [Self::refined].
    /// - If this layer is not the apex, then [Self::subdivided] is recursively passed as `source` to the next layer.
    pub(crate) fn build(
        &mut self,
        source: &[Real],
        downsample_smoothing: &ConvolutionFilter,
        upsample_smoothing: &ConvolutionFilter,
    ) {
        // Downsample from source.
        self.subdivided.downsample(source);
        self.subdivided.convolve(downsample_smoothing);

        // Upsample from the next layer's subdivided.
        self.refined.upsample(&self.subdivided);
        self.refined.convolve(upsample_smoothing);

        // Extract detail coefficients.
        self.detail_coefficients
            .extract_from_slices(source, &self.refined);

        //  Recurse method to next layer.
        if let Some(next_layer) = &mut self.next_layer {
            next_layer.build(&self.subdivided, downsample_smoothing, upsample_smoothing);
        }
    }

    /// Performs the second stage of the smoothing algorithm recursively.
    /// Should be called after [Self::build].
    ///
    /// Calls [DetailCoefficient::process] and propagates the method recursively.
    pub(crate) fn process(&mut self) {
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
    pub(crate) fn rebuild(&mut self, refinement_smoothing: &ConvolutionFilter) -> &[Real] {
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
impl PyramidLayer {
    pub(super) fn get_subdivided(&self) -> &ConvolutionCache {
        &self.subdivided
    }

    pub(super) fn get_refined(&self) -> &ConvolutionCache {
        &self.refined
    }

    pub(super) fn get_detail_coefficients(&self) -> &DetailCoefficients {
        &self.detail_coefficients
    }

    pub(super) fn get_next_layer(&self) -> Option<&Box<PyramidLayer>> {
        self.next_layer.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Deref;

    use super::*;
    use crate::{
        pulse_detection::window::convolution_filter::KernelType, test_data::assert_iters_equal,
    };
    use num::Integer;

    fn assert_layer_sizes(layer_level: &PyramidLayer, size: usize, _padding: usize) {
        assert_eq!(layer_level.detail_coefficients.deref().len(), size);
        assert_eq!(layer_level.refined.deref().len(), size);
    }

    #[test]
    fn test_two_layers() {
        let settings = LayerProcessingSettings::default();
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![0.0, 0.0, 0.0]));

        let mut base = PyramidLayer::new(
            vec![settings],
            gamma.kernel_size() / 2,
            alpha.kernel_size() / 2,
        )
        .unwrap();
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

        let mut base = PyramidLayer::new(
            vec![LayerProcessingSettings::default(); 2],
            gamma.kernel_size() / 2,
            alpha.kernel_size() / 2,
        )
        .unwrap();
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
    fn test_layer_unconvolved() {
        let mut layer = PyramidLayer::new(vec![LayerProcessingSettings::default()], 0, 0).unwrap();
        layer.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.build(DATA.as_slice(), &alpha, &gamma);

        let output: &[Real] = &layer.subdivided;
        let expected_data = DATA.iter().step_by(2);
        assert_iters_equal(output.iter(), expected_data.into_iter());

        let output: &[Real] = &layer.refined;
        let expected_data = DATA
            .iter()
            .enumerate()
            .map(|(i, v): (usize, _)| if i.is_even() { v } else { &0.0 });
        assert_iters_equal(output.iter(), expected_data);
    }

    #[test]
    fn test_two_layer_unconvolved_process() {
        let mut layer =
            PyramidLayer::new(vec![LayerProcessingSettings::default(); 2], 0, 0).unwrap();
        layer.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.build(DATA.as_slice(), &alpha, &gamma);

        let output: &[Real] = &layer.subdivided;
        let expected_data = DATA.iter().step_by(2);
        assert_iters_equal(output.iter(), expected_data);

        let output: &[Real] = &layer.refined;
        let expected_data = DATA
            .iter()
            .enumerate()
            .map(|(i, v): (usize, _)| if i.is_even() { v } else { &0.0 });
        assert_iters_equal(output.iter(), expected_data);

        match layer.next_layer {
            None => unreachable!(),
            Some(layer) => {
                let output: &[Real] = &layer.subdivided;
                let expected_data = DATA.iter().step_by(4);
                assert_iters_equal(output.iter(), expected_data);

                let output: &[Real] = &layer.refined;
                let expected_data = DATA
                    .iter()
                    .step_by(2)
                    .enumerate()
                    .map(|(i, v): (usize, _)| if i.is_even() { v } else { &0.0 });
                assert_iters_equal(output.iter(), expected_data);

                assert!(layer.next_layer.is_none());
            }
        }
    }

    #[test]
    fn test_two_layer_unconvolved_rebuild() {
        let mut layer =
            PyramidLayer::new(vec![LayerProcessingSettings::default(); 2], 0, 0).unwrap();
        layer.init_size(SIZE);
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(vec![1.0]));
        layer.build(DATA.as_slice(), &alpha, &gamma);
        layer.rebuild(&alpha);

        let output: &[Real] = &layer.refined;
        let expected_data = DATA.iter();
        assert_iters_equal(output.iter(), expected_data);

        match layer.next_layer {
            None => unreachable!(),
            Some(layer) => {
                let output: &[Real] = &layer.refined;
                let expected_data = DATA.iter().step_by(2);
                assert_iters_equal(output.iter(), expected_data);

                assert!(matches!(layer.next_layer, None));
            }
        }
    }
}
