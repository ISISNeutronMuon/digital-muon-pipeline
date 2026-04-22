//! Provides objects for persisting state for the multiscaling smoothing algorithm.
use crate::{
    channels::algorithm_states::{AlgorithmState, DifferentialThresholdDiscriminatorState, SmoothingDetectorState, ThresholdDetectorState},
    parameters::{MultiscalingDetectorMethod, MultiscalingDetectorParameters},
    pulse_detection::{
        Real,
        window::{
            SliceWindow,
            convolution_filter::{ConvolutionFilter, KernelType},
            fft_inverse::FftInverse,
            pyramid::PyramidLayer,
        },
    },
};
use digital_muon_common::Intensity;
use num::complex::ComplexFloat;

/// Encapsulates settings and objects specific to the method used by the multiscaling algorithm.
#[derive(Clone)]
pub(crate) enum MultiscalingMethodAlgorithmState {
    /// Encapsulates channel state used by the Fixed Threshold algorithm.
    FixedThreshold(ThresholdDetectorState),
    /// Encapsulates channel state used by the Differential Threshold algorithm.
    DifferentialThreshold(DifferentialThresholdDiscriminatorState),
    /// Encapsulates channel state used by the Smoothing algorithm.
    Smoothing(SmoothingDetectorState),
}

impl MultiscalingMethodAlgorithmState {
    /// Creates a new `ChannelAlgorithmState` object defined from `mode`. The state object is specific to the detector chosen.
    ///
    /// # Parameters
    /// - mode: the `Mode` enum to create the state object from.
    pub(crate) fn new(mode: &MultiscalingDetectorMethod) -> Self {
        match mode {
            MultiscalingDetectorMethod::FixedThresholdDiscriminator(parameters) => Self::FixedThreshold(ThresholdDetectorState::new(parameters)),
            MultiscalingDetectorMethod::DifferentialThresholdDiscriminator(parameters) => {
                Self::DifferentialThreshold(DifferentialThresholdDiscriminatorState::new(
                    parameters,
                ))
            }
            MultiscalingDetectorMethod::SmoothingDetector(parameters) => {
                Self::Smoothing(SmoothingDetectorState::new(parameters))
            }
        }
    }
}

/// Encapsulates settings used to during the processing phase of the multiscaling algorithm.
///
/// Processing consists of denoising, enhancing, and multiplication, operating in that order.
#[derive(Default, Clone)]
pub(crate) struct LayerProcessingSettings {
    /// If present, then absolute trace values below this threshold are set to zero.
    pub(crate) denoise_threshold: Option<Real>,
    /// If present, then (signed) trace values above this threshold (first value) are multiplied by the factor (second value).
    pub(crate) enhance_threshold_factor: Option<(Real, Real)>,
    /// If present, then trace values are multiplied by this factor.
    pub(crate) multiply_factor: Option<Real>,
}

/// Encapsulates all settings and objects in the smoothing algorithm which persist across digitiser messages.
#[derive(Clone)]
pub(crate) struct MultiscalingDetectorState {
    /// Smoothing filter to apply after downsampling.
    pub(crate) downsample_smoothing: ConvolutionFilter,
    /// Smoothing filter to apply after upsampling.
    pub(crate) upsample_smoothing: ConvolutionFilter,
    /// This cache is persisted to avoid reallocations on every channel trace.
    pub(crate) cache: MultiscalingDetectorCache,
    /// The state of the underlying algorithm.
    pub(crate) method_state: MultiscalingMethodAlgorithmState,
}

impl MultiscalingDetectorState {
    /// Creates new instance of detector state.
    ///
    /// # Parameters
    /// - parameters: settings given in the command line.
    pub(crate) fn new(parameters: &MultiscalingDetectorParameters) -> Self {
        // FIXME: Could this be handled directly by Clap? Or if not, moved elsewhere?
        if parameters.denoise {
            assert_eq!(
                parameters.number_of_layers,
                parameters.denoise_thresholds.len()
            );
        }
        if parameters.enhance {
            assert_eq!(
                parameters.number_of_layers,
                parameters.enhance_thresholds.len()
            );
            assert_eq!(
                parameters.number_of_layers,
                parameters.enhance_factors.len()
            );
        }
        if parameters.multiply {
            assert_eq!(
                parameters.number_of_layers,
                parameters.multiply_factors.len()
            );
        }

        // Extract layer settings from cli args.
        let layers_settings = (0..parameters.number_of_layers)
            .map(|layer| LayerProcessingSettings {
                denoise_threshold: parameters
                    .denoise
                    .then(|| parameters.denoise_thresholds[layer]),
                enhance_threshold_factor: parameters.enhance.then(|| {
                    (
                        parameters.enhance_thresholds[layer],
                        parameters.enhance_factors[layer],
                    )
                }),
                multiply_factor: parameters
                    .multiply
                    .then(|| parameters.multiply_factors[layer]),
            })
            .collect();

        // Create `upsample_smoothing_coefs` from `downsample_smoothing_coefs`.
        let downsample_smoothing_coefs = parameters.downsampling_smoothing.clone();
        let fft = FftInverse::new(
            parameters.fft_padding,
            parameters.fft_truncation,
            parameters.smoothing_support.clone(),
            ComplexFloat::recip,
        );
        let mut upsample_smoothing_coefs = vec![Default::default(); parameters.fft_truncation];
        fft.apply_to_slice(
            downsample_smoothing_coefs.as_slice(),
            upsample_smoothing_coefs.as_mut_slice(),
        );

        // Create convolution filters.
        let downsample_smoothing =
            ConvolutionFilter::new(KernelType::ManualCoefficients(downsample_smoothing_coefs));
        let upsample_smoothing =
            ConvolutionFilter::new(KernelType::ManualCoefficients(upsample_smoothing_coefs));

        let method_state = MultiscalingMethodAlgorithmState::new(&parameters.method);
        let cache = MultiscalingDetectorCache {
            pyramid: PyramidLayer::new(
                layers_settings,
                downsample_smoothing.kernel_size() >> 1,
                upsample_smoothing.kernel_size() >> 1,
            )
            .expect("Pyramid should be configured correctly, this should never fail."),
            ..Default::default()
        };
        Self {
            downsample_smoothing,
            upsample_smoothing,
            method_state,
            cache,
        }
    }
}

impl AlgorithmState for MultiscalingDetectorState {
    #[tracing::instrument(skip_all, level = "trace")]
    fn find_events(
        &mut self,
        trace: impl Clone + ExactSizeIterator<Item = Real> + DoubleEndedIterator,
        polarity_sign: Real,
        baseline: Real,
    ) -> (Vec<usize>, Vec<Intensity>) {
        self.cache.ensure_cache_lengths(trace.len());
        self.cache.write_input_values(trace);

        // Apply three stages of the pyramid algorithm.
        self.cache.pyramid.build(
            &self.cache.input_values,
            &self.downsample_smoothing,
            &self.upsample_smoothing,
        );
        self.cache.pyramid.process();
        let smoothed_trace = self.cache.pyramid.rebuild(&self.upsample_smoothing).iter().cloned();

        // Pass the smoothed trace on to the method.
        let (index, mut intensity) = match &mut self.method_state {
            MultiscalingMethodAlgorithmState::FixedThreshold(state) => state.find_events(smoothed_trace, polarity_sign, baseline),
            MultiscalingMethodAlgorithmState::DifferentialThreshold(state) => state.find_events(smoothed_trace, polarity_sign, baseline),
            MultiscalingMethodAlgorithmState::Smoothing(state) => state.find_events(smoothed_trace,polarity_sign,baseline),
        };
        // Set the intensity to the trace value corresponding to the index.
        // The intensity output from the underlying method is potentially inaccurate
        // due to the enhance and muliply stages of the processessing phase.
        for (&index, val) in index.iter().zip(intensity.iter_mut()) {
            *val = *self.cache.input_values.get(index).expect("Element should exist, this should never fail.") as Intensity
        }
        (index, intensity)
    }
}

/// Memory which is used in the multiscaling algorithm.
/// These are persisted and overwritten each channel trace,
/// to avoid repeated memory reallocation.
#[derive(Default, Clone)]
pub(crate) struct MultiscalingDetectorCache {
    /// Value of `trace.len()`.
    expected_size: Option<usize>,
    /// Memory in which to write the pre-convolution trace data.
    pub(crate) input_values: Vec<Real>,
    /// Filter which to apply to `input_values`.
    pub(crate) pyramid: Box<PyramidLayer>,
}

impl MultiscalingDetectorCache {
    /// Ensures the value caches are of sufficient length for the message.
    /// If the fields are too small, they are resized.
    /// # Parameters
    /// - size: the minimum length of the cache's vectors.
    pub(crate) fn ensure_cache_lengths(&mut self, input_size: usize) {
        // FIXME: Should there be some sort of check for absurdly big trace sizes?
        if self
            .expected_size
            .is_none_or(|expected_size| input_size != expected_size)
        {
            self.expected_size = Some(input_size);
            self.input_values.resize(input_size, Default::default());
            self.pyramid.init_size(input_size);
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



#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        channels::algorithm_states::MultiscalingDetectorState,
        parameters::{
            FixedThresholdDiscriminatorParameters, MultiscalingDetectorMethod,
            MultiscalingDetectorParameters,
        },
        test_data::{assert_iters_approx_equal, assert_iters_equal, pyramid::INPUT},
    };

    #[test]
    fn test_pyramid() {
        let mut state = MultiscalingDetectorState::new(&MultiscalingDetectorParameters {
            downsampling_smoothing: vec![0.125, 0.5, 0.75, 0.5, 0.125],
            smoothing_support: vec![-2, -1, 0, 1, 2],
            fft_padding: 200,
            fft_truncation: 5,
            number_of_layers: 4,
            denoise: true,
            denoise_thresholds: vec![2.0, 5.0, 7.0, 20.0],
            enhance: true,
            enhance_thresholds: vec![40.0, 30.0, 35.0, 50.0],
            enhance_factors: vec![1.5, 1.375, 1.25, 1.125],
            multiply: true,
            multiply_factors: vec![1.0, 0.7, 0.2, 0.1],
            method: MultiscalingDetectorMethod::FixedThresholdDiscriminator(
                FixedThresholdDiscriminatorParameters {
                    threshold: 10.0,
                    duration: 2,
                    cool_off: 0,
                },
            ),
        });
        let input = INPUT.map(|x| x * 1000.0).into_iter();
        let (times, intensities) = state.find_events(
            input, 1.0, 0.0);
        let times = times.into_iter().map(|x| x as Real).collect::<Vec<_>>();
        let intensities = intensities
            .into_iter()
            .map(|x| x as Real)
            .collect::<Vec<_>>();
        let expected_times = [11.0, 27.0, 36.0, 43.0, 59.0, 126.0];
        let expected_intensities = [41.0, 69.0, 25.0, 22.0, 14.0, 112.0];
        assert_iters_equal(times.iter(), expected_times.iter());
        assert_iters_approx_equal(intensities.iter(), expected_intensities.iter());
    }
}
