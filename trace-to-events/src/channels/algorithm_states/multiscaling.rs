use crate::{
    channels::algorithm_states::{DifferentialThresholdDiscriminatorState, SmoothingDetectorState},
    parameters::{MultiscalingDetectorMethod, MultiscalingDetectorParameters},
    pulse_detection::{
        Real,
        threshold_detector::ThresholdDuration,
        window::{
            SliceWindow,
            convolution_filter::{ConvolutionFilter, KernelType},
            fft_inverse::FftInverse,
            pyramid::PyramidFilter,
        },
    },
};
use num::complex::ComplexFloat;

/// Encapsulates settings and objects specific to the method used by the multiscaling algorithm.
#[derive(Clone)]
pub(crate) enum MultiscalingMethodAlgorithmState {
    /// Encapsulates channel state used by the Fixed Threshold algorithm.
    FixedThreshold { parameters: ThresholdDuration },
    /// Encapsulates channel state used by the Differential Threshold algorithm.
    DifferentialThreshold(DifferentialThresholdDiscriminatorState),
    /// Encapsulates channel state used by the Smoothing algorithm.
    Smoothing(SmoothingDetectorState),
}

impl MultiscalingMethodAlgorithmState {
    /// Creates a new `ChannelAlgorithmState` object defined from `mode`. The state object is specific to the detector chosen.
    /// # Parameters
    /// - mode: the `Mode` enum to create the state object from.
    pub(crate) fn new(mode: &MultiscalingDetectorMethod) -> Self {
        match mode {
            MultiscalingDetectorMethod::FixedThresholdDiscriminator(parameters) => {
                Self::FixedThreshold {
                    parameters: ThresholdDuration {
                        threshold: parameters.threshold,
                        duration: parameters.duration,
                        cool_off: parameters.cool_off,
                    },
                }
            }
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

#[derive(Default, Clone)]
pub(crate) struct LayerProcessingSettings {
    pub(crate) denoise_threshold: Option<Real>,
    pub(crate) enhance_threshold_factor: Option<(Real, Real)>,
    pub(crate) multiply_factor: Option<Real>,
}

/// Encapsulates all settings and objects in the smoothing algorithm which persist across digitiser messages.
#[derive(Clone)]
pub(crate) struct MultiscalingDetectorState {
    /// This cache is persisted to avoid reallocations on every channel trace.
    pub(crate) cache: MultiscalingDetectorCache,
    /// The state of the underlying algorithm.
    pub(crate) method_state: MultiscalingMethodAlgorithmState,
}

impl MultiscalingDetectorState {
    pub(crate) fn new(parameters: &MultiscalingDetectorParameters) -> Self {
        let layers_settings = (0..parameters.number_of_layers)
            .map(|layer| LayerProcessingSettings {
                denoise_threshold: parameters
                    .denoise
                    .then(|| parameters.denoise_thresholds[layer] as Real),
                enhance_threshold_factor: parameters.enhance.then(|| {
                    (
                        parameters.enhance_thresholds[layer] as Real,
                        parameters.enhance_factors[layer] as Real,
                    )
                }),
                multiply_factor: parameters
                    .multiply
                    .then(|| parameters.multiply_factors[layer] as Real), // FIXME
            })
            .collect();
        let subdivide_smoothing_coefs = parameters.subdivision_smoothing.clone();
        let fft = FftInverse::new(
            200,
            20,
            parameters.smoothing_support.clone(),
            ComplexFloat::recip,
        );
        let mut refinement_smoothing_coefs = vec![0.0; 20];
        fft.apply_to_slice(
            subdivide_smoothing_coefs.as_slice(),
            refinement_smoothing_coefs.as_mut_slice(),
        );

        let subdivide_smoothing =
            ConvolutionFilter::new(KernelType::ManualCoefficients(subdivide_smoothing_coefs));
        let refinement_smoothing =
            ConvolutionFilter::new(KernelType::ManualCoefficients(refinement_smoothing_coefs));
        let method_state = MultiscalingMethodAlgorithmState::new(&parameters.method);
        Self {
            method_state,
            cache: MultiscalingDetectorCache {
                pyramid: PyramidFilter::new(
                    layers_settings,
                    refinement_smoothing,
                    subdivide_smoothing,
                ).expect("Pyramid should be configured correctly, this should never fail."),
                ..Default::default()
            },
        }
    }
}

/// Memory which is used in the smoothing algorithm.
/// These are persisted and overwritten each channel trace,
/// to avoid repeated memory reallocation.
#[derive(Default, Clone)]
pub(crate) struct MultiscalingDetectorCache {
    /// Value of `trace.len()`
    expected_size: Option<usize>,
    /// Memory in which to write the pre-convolution trace data.
    pub(crate) input_values: Vec<Real>,
    ///
    pub(crate) pyramid: PyramidFilter,
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
