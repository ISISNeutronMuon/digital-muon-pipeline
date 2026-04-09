
#[derive(Default, Clone)]
pub(crate) struct LayerProcessingSettings {
    pub(crate) denoise_threshold: Option<Real>,
    pub(crate) enhance_threshold_factor: Option<(Real, Real)>,
    pub(crate) multiply_factor: Option<Real>,
}

/// Encapsulates all settings and objects in the smoothing algorithm which persist across digitiser messages.
#[derive(Clone)]
struct MultiscalingDetectorState {
    /// Parameters for the smoothing detector.
    parameters: MultiscalingDetectorParameters,
    /// This cache is persisted to avoid reallocations on every channel trace.
    cache: MultiscalingDetectorCache,
}

impl MultiscalingDetectorState {
    fn new(parameters: &MultiscalingDetectorParameters) -> Self {
        let layers_settings = (0..parameters.number_of_layers).map(|layer|LayerProcessingSettings {
            denoise_threshold: parameters.denoise.then(||parameters.scales_denoise[layer] as Real),
            enhance_threshold_factor: parameters.enhance.then(||(parameters.enhancement_threshold[layer] as Real, parameters.enhancement_factor[layer] as Real)),
            multiply_factor: parameters.multiply.then(||parameters.enhance_scales[layer] as Real), // FIXME
        }).collect();
        let subdivide_smoothing_coefs = parameters.alpha.clone();
        let fft = FftInverse::new(subdivide_smoothing_coefs.len(), subdivide_smoothing_coefs.len(), parameters.support.clone(), ComplexFloat::recip);
        let mut refinement_smoothing_coefs = vec![0.0; subdivide_smoothing_coefs.len()];
        fft.apply_to_slice(subdivide_smoothing_coefs.as_slice(), refinement_smoothing_coefs.as_mut_slice());

        let subdivide_smoothing = ConvolutionFilter::new(KernelType::ManualCoefficients(subdivide_smoothing_coefs));
        let refinement_smoothing = ConvolutionFilter::new(KernelType::ManualCoefficients(refinement_smoothing_coefs));
        Self {
            parameters: parameters.clone(),
            cache: MultiscalingDetectorCache {
                expected_sample_time: None,
                pyramid: PyramidFilter::new(layers_settings, refinement_smoothing, subdivide_smoothing),
                ..Default::default()
            }
        }
    }
}

/// Memory which is used in the smoothing algorithm.
/// These are persisted and overwritten each channel trace,
/// to avoid repeated memory reallocation.
#[derive(Default, Clone)]
pub(super) struct MultiscalingDetectorCache {
    /// Value of `sample_time`
    expected_sample_time: Option<Real>,
    /// Value of `trace.len()`
    expected_size: Option<usize>,
    /// Memory in which to write the time bin values.
    pub(super) time: Vec<Real>,
    /// Memory in which to write the pre-convolution trace data.
    pub(super) input_values: Vec<Real>,
    ///
    pub(super) pyramid: PyramidFilter,
}

impl MultiscalingDetectorCache {
    /// Refreshes the `time` vector if and only if the size of the vector changes, or the `sample_time` field.
    /// # Parameters
    /// - size: the intended size of the `time` vector.
    /// - sample_time: the intended `sample_time`, defining the scale of the time-series.
    pub(super) fn ensure_time_data_written(&mut self, size: usize, sample_time: Real) {
        if size != self.time.len()
            || self
                .expected_sample_time
                .is_some_and(|current_sample_time| current_sample_time != sample_time)
        {
            self.time = (0..size).map(|t| t as Real * sample_time).collect();
            self.expected_sample_time = Some(sample_time);
        }
    }

    /// Ensures the value caches are of sufficient length for the message.
    /// If the fields are too small, they are resized.
    /// # Parameters
    /// - size: the minimum length of the cache's vectors.
    pub(super) fn ensure_cache_lengths(&mut self, input_size: usize) {
        // FIXME: Should there be some sort of check for absurdly big trace sizes?
        if self.expected_size.is_none_or(|expected_size|input_size != expected_size) {
            self.expected_size = Some(input_size);
            self.pyramid.init_size(input_size);
        }
    }

    /// Write to the `input_values` field from an iterator over the appropriately padded trace values.
    ///
    /// This should not be called unless `Self::ensure_cache_lengths` has been called with the appropriate `size` value.
    /// # Parameters
    /// - input: iterator from which the `input_values` field is written.
    pub(super) fn write_input_values(&mut self, input: impl Iterator<Item = Real> + Clone) {
        for (i, v) in input.enumerate() {
            self.input_values[i] = v;
        }
    }
}