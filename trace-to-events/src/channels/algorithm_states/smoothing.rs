//! Provides objects for persisting state for the second-order smoothing algorithm.
use digital_muon_common::{Intensity, Time};

use crate::{
    channels::algorithm_states::AlgorithmState, parameters::SmoothingDetectorParameters, pulse_detection::{
        EventsIterable, Real, detectors::{local_arg_min_detector::LocalArgMinDetector, region_detector::RegionDetector}, iterators::PaddingIterable, utils::{global_arg_min, std_dev}, window::{SliceWindow, convolution_filter::{ConvolutionFilter, KernelType}}
    }
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

impl AlgorithmState for SmoothingDetectorState {
    #[tracing::instrument(skip_all, level = "trace", fields(std_dev, num_regions))]
    fn find_events(
        &mut self,
        trace: impl Clone + ExactSizeIterator<Item = Real> + DoubleEndedIterator,
        sample_time: Real,
        polarity_sign: Real,
        baseline: Real,
    ) -> (Vec<Time>, Vec<Intensity>) {
        self.cache.ensure_time_data_written(trace.len(), sample_time);
        // Get the radius of the kernel by right-bitshifting the size by one
        // i.e. divide by 2, and round-down.
        let kernel_radius = self.fin_diff_gaussian.kernel_size() >> 1;
        let padded = trace
            .clone()
            .map(|v| polarity_sign * (v as Real - baseline))
            .pad_reflect(kernel_radius, kernel_radius);
        self.cache.ensure_cache_lengths(trace.len() + self.fin_diff_gaussian.kernel_size(), trace.len());
        self.cache.write_input_values(padded);

        self.fin_diff_gaussian.apply_to_slice(
            self.cache.input_values.as_slice(),
            self.cache.output_values.as_mut_slice(),
        );

        let percentile = ((trace.len() as Real * self.parameters.noise_centile) / 100.0) as usize;
        let noise_std = std_dev(&self.cache.output_values[percentile..])
            .expect("std_dev should exist, this should never fail.");
        tracing::Span::current().record("std_dev", noise_std);

        let output_iter = self.cache.output_values.iter().cloned().enumerate();

        let regions = output_iter
            .clone()
            .events(RegionDetector::new(
                -noise_std * self.parameters.nsig_noise,
                self.parameters.min_size,
            ))
            .collect::<Vec<_>>();
        tracing::Span::current().record("num_regions", regions.len());

        let pulses = regions
            .into_iter()
            .flat_map(|region| {
                let region_iter = output_iter.clone().take(region.1).skip(region.0);
                if let Some(use_local_for_sizes_ge) = self.parameters.use_local_for_sizes_ge
                    && region_iter.len() >= use_local_for_sizes_ge
                {
                    region_iter
                        .events(LocalArgMinDetector::default())
                        .collect::<Vec<_>>()
                } else {
                    vec![global_arg_min(region_iter)]
                }
            })
            .collect::<Vec<_>>();

        let mut times = Vec::<Time>::new();
        let mut voltages = Vec::<Intensity>::new();
        for time in pulses {
            times.push(time as Time);
            voltages.push(trace.clone().nth(time).expect("") as Intensity);
        }
        (times, voltages)
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
