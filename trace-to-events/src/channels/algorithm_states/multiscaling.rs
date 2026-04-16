//! Provides objects for persisting state for the multiscaling smoothing algorithm.
use crate::{
    channels::algorithm_states::{DifferentialThresholdDiscriminatorState, SmoothingDetectorState},
    parameters::{MultiscalingDetectorMethod, MultiscalingDetectorParameters},
    pulse_detection::{
        Real,
        threshold_detector::ThresholdDuration,
        window::{
            SliceWindow, convolution_filter::{ConvolutionFilter, KernelType}, fft_inverse::FftInverse, pyramid::PyramidLayer
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
    /// 
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
    pub(crate) subdivide_smoothing: ConvolutionFilter,
    /// Smoothing filter to apply after upsampling.
    pub(crate) refinement_smoothing: ConvolutionFilter,
    /// This cache is persisted to avoid reallocations on every channel trace.
    pub(crate) cache: MultiscalingDetectorCache,
    /// The state of the underlying algorithm (boxed to placate `cargo clippy`).
    pub(crate) method_state: Box<MultiscalingMethodAlgorithmState>,
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

        // Create `refinement_smoothing_coefs` from `subdivide_smoothing_coefs`.
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

        // Create convolution filters.
        let subdivide_smoothing =
            ConvolutionFilter::new(KernelType::ManualCoefficients(subdivide_smoothing_coefs));
        let refinement_smoothing =
            ConvolutionFilter::new(KernelType::ManualCoefficients(refinement_smoothing_coefs));

        let method_state = Box::new(MultiscalingMethodAlgorithmState::new(&parameters.method));
        let cache = MultiscalingDetectorCache {
            pyramid: PyramidLayer::new(
                layers_settings,
                subdivide_smoothing.kernel_size() >> 1,
                refinement_smoothing.kernel_size() >> 1,
            )
            .expect("Pyramid should be configured correctly, this should never fail."),
            ..Default::default()
        };
        Self {
            refinement_smoothing,
            subdivide_smoothing,
            method_state,
            cache
        }
    }
}

/// Memory which is used in the smoothing algorithm.
/// These are persisted and overwritten each channel trace,
/// to avoid repeated memory reallocation.
#[derive(Default, Clone)]
pub(crate) struct MultiscalingDetectorCache {
    /// Value of `trace.len()`.
    expected_size: Option<usize>,
    /// Memory in which to write the pre-convolution trace data.
    pub(crate) input_values: Vec<Real>,
    /// Filter which to apply to `input_values`.
    pub(crate) pyramid: Box::<PyramidLayer>,
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
