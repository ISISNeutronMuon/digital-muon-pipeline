use crate::{
    parameters::SmoothingDetectorParameters,
    pulse_detection::{
        Real,
        window::convolution_filter::{ConvolutionFilter, KernelType},
    },
};

/// Encapsulates all settings and objects in the smoothing algorithm which persist across digitiser messages.
#[derive(Clone)]
pub(crate) struct SmoothingDetectorState {
    /// Parameters for the smoothing detector.
    pub(crate) parameters: SmoothingDetectorParameters,
    /// Composite Kernel uses to smooth the trace and take the second derivative.
    pub(crate) fin_diff_gaussian: ConvolutionFilter,
    /// This cache is persisted to avoid reallocations on every channel trace.
    pub(crate) cache: SmoothingDetectorCache,
}

impl SmoothingDetectorState {
    pub(crate) fn new(parameters: &SmoothingDetectorParameters) -> Self {
        Self {
            parameters: parameters.clone(),
            fin_diff_gaussian: ConvolutionFilter::new(KernelType::Composition {
                left: Box::new(KernelType::FiniteDifference { order: 2 }),
                right: Box::new(KernelType::Gaussian {
                    sigma: parameters.kernel_sigma,
                }),
            }),
            cache: Default::default(),
        }
    }
}

/// Memory which is used in the smoothing algorithm.
/// These are persisted and overwritten each channel trace,
/// to avoid repeated memory reallocation.
#[derive(Default, Clone)]
pub(crate) struct SmoothingDetectorCache {
    /// Value of `sample_time`
    expected_sample_time: Option<Real>,
    /// Memory in which to write the time bin values.
    time: Vec<Real>,
    /// Memory in which to write the pre-convolution trace data.
    pub(crate) input_values: Vec<Real>,
    /// Memory in which the convolution window should write its output.
    pub(crate) output_values: Vec<Real>,
}

impl SmoothingDetectorCache {
    /// Refreshes the `time` vector if and only if the size of the vector changes, or the `sample_time` field.
    /// # Parameters
    /// - size: the intended size of the `time` vector.
    /// - sample_time: the intended `sample_time`, defining the scale of the time-series.
    pub(crate) fn ensure_time_data_written(&mut self, size: usize, sample_time: Real) {
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
    pub(crate) fn ensure_cache_lengths(&mut self, input_size: usize, output_size: usize) {
        // FIXME: Should there be some sort of check for absurdly big trace sizes?
        if input_size > self.input_values.len() {
            self.input_values.resize(input_size, Default::default());
        }

        if output_size > self.output_values.len() {
            self.output_values.resize(output_size, Default::default());
        }
    }

    /// Write to the `input_values` field from an iterator over the appropriately padded trace values.
    ///
    /// This should not be called unless `Self::ensure_cache_lengths` has been called with the appropriate `size` value.
    /// # Parameters
    /// - input: iterator from which the `input_values` field is written.
    pub(crate) fn write_input_values(&mut self, input: impl Iterator<Item = Real> + Clone) {
        for (i, v) in input.enumerate() {
            self.input_values[i] = v;
        }
    }
}
