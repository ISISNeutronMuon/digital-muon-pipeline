//!
//! # Example
//!
//! The following example applies a smoothing window of length five to a raw
//! data stream.
//! Note that a [SmoothingWindow] outputs a [Stats] type, so we need to extract
//! the [Stats::mean] value to convert to a scalar stream.
//! ```rust
//!     let smoothed = raw
//!        .window(SmoothingWindow::new(5))
//!        .map(|(i, stats)| (i, stats.mean));
//! ```
//use crate::pulse_detection::window::SliceWindow;
mod layer;
mod traces;

use super::Real;
use crate::pulse_detection::window::{
    convolution_filter::ConvolutionFilter, pyramid::layer::LayerProcessingSettings,
};
use layer::{Layer};
use traces::{ConvolutionCache, DetailCoefficients};


fn downsample(input: &[Real], output: &mut [Real], padding: usize) {
    let size = input.len();
    for (i, o) in output.iter_mut().skip(padding).take(size / 2).enumerate() {
        *o = *input
            .get(2 * i)
            .expect("Slice element should exist, this should never fail.");
    }

    println!("Downsampling");
    println!("  input = {input:?}");
    println!("  output = {output:?}\n");
}

fn upsample(input: &[Real], output: &mut [Real], padding: usize) {
    for (i, value) in input.iter().enumerate() {
        *output
            .get_mut(2 * i + padding)
            .expect("Slice element should exist, this should never fail.") = *value;
    }

    println!("Upsampling");
    println!("  input = {input:?}");
    println!("  output = {output:?}\n");
}

#[derive(Default, Clone)]
pub(crate) struct PyramidFilter {
    alpha: ConvolutionFilter,
    gamma: ConvolutionFilter,
    pyramid_base: Layer,
}

impl PyramidFilter {
    pub(crate) fn new(
        layer_settings: Vec<LayerProcessingSettings>,
        size: usize,
        alpha: ConvolutionFilter,
        gamma: ConvolutionFilter,
    ) -> Self {
        let subdivide_padding = gamma.kernel_size() / 2;
        let refined_padding = alpha.kernel_size() / 2;
        let pyramid_base = Layer::new(size, layer_settings, subdivide_padding, refined_padding);
        PyramidFilter {
            alpha,
            gamma,
            pyramid_base
        }
    }

    fn apply_to_slice<'a>(&mut self, input: &[Real], output: &mut [Real]) {
        self.pyramid_base.process(input, &self.alpha, &self.gamma);
        self.pyramid_base.rebuild(&self.alpha, Some(output));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse_detection::{
        iterators::{PaddingIterable, WindowIterable},
        window::{SliceWindow, convolution_filter::KernelType, fft_inverse::FftInverse},
    };
    use rustfft::num_complex::{Complex, ComplexFloat};
    use assert_approx_eq::assert_approx_eq;
    use digital_muon_common::Intensity;

    // number of data points
    const NX: usize = 85;

    // data y values
    const Y: [f64; NX] = [
        0.0299212598425197,
        0.0299212598425197,
        0.0299212598425197,
        0.0299212598425197,
        0.04566929133858272,
        0.0771653543307087,
        0.10866141732283469,
        0.1283464566929134,
        0.1283464566929134,
        0.12440944881889765,
        0.11653543307086617,
        0.10472440944881894,
        0.09685039370078741,
        0.08503937007874018,
        0.0771653543307087,
        0.06929133858267722,
        0.06535433070866142,
        0.05748031496062994,
        0.04960629921259846,
        0.04566929133858272,
        0.04566929133858272,
        0.04173228346456698,
        0.03779527559055118,
        0.0299212598425197,
        0.0299212598425197,
        0.0299212598425197,
        0.0299212598425197,
        0.0299212598425197,
        0.025984251968503957,
        0.022047244094488216,
        0.018110236220472475,
        0.018110236220472475,
        0.022047244094488216,
        0.0299212598425197,
        0.04173228346456698,
        0.06141732283464568,
        0.08110236220472444,
        0.09291338582677167,
        0.09291338582677167,
        0.09685039370078741,
        0.09685039370078741,
        0.09291338582677167,
        0.08110236220472444,
        0.06929133858267722,
        0.05748031496062994,
        0.0535433070866142,
        0.04960629921259846,
        0.04566929133858272,
        0.04173228346456698,
        0.03385826771653544,
        0.0299212598425197,
        0.025984251968503957,
        0.025984251968503957,
        0.022047244094488216,
        0.018110236220472475,
        0.018110236220472475,
        0.018110236220472475,
        0.018110236220472475,
        0.018110236220472475,
        0.018110236220472475,
        0.018110236220472475,
        0.018110236220472475,
        0.018110236220472475,
        0.018110236220472475,
        0.014173228346456734,
        0.018110236220472475,
        0.018110236220472475,
        0.018110236220472475,
        0.014173228346456734,
        0.014173228346456734,
        0.014173228346456734,
        0.018110236220472475,
        0.018110236220472475,
        0.014173228346456734,
        0.014173228346456734,
        0.014173228346456734,
        0.018110236220472475,
        0.018110236220472475,
        0.018110236220472475,
        0.018110236220472475,
        0.018110236220472475,
        0.014173228346456734,
        0.014173228346456734,
        0.014173228346456734,
        0.010236220472440993,
    ];
    /*
    #[test]
    fn test_small_data() {
        //input = []
        let alpha_coefs = vec![0.125, 0.5, 0.75, 0.5, 0.125];
        let support = vec![-2, -1, 0, 1, 2];
        let gamma_coefs = reverse(&alpha_coefs, &support, 200, 101).unwrap();
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(alpha_coefs));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(gamma_coefs));
    }
    */

    #[test]
    fn test_gaussian_convolution() {
        let alpha_coefs = vec![0.125, 0.5, 0.75, 0.5, 0.125];
        let support = vec![-2, -1, 0, 1, 2];
        let mut gamma_coefs = vec![0.0; 5];

        let fft = FftInverse::new(50, 6, support.clone(), Complex::recip);
        fft.apply_to_slice(alpha_coefs.as_slice(), gamma_coefs.as_mut_slice());
        let alpha = ConvolutionFilter::new(KernelType::ManualCoefficients(alpha_coefs));
        let gamma = ConvolutionFilter::new(KernelType::ManualCoefficients(gamma_coefs));

        let mut pyramid = PyramidFilter::new(vec![LayerProcessingSettings {
            denoise_threshold: Some(0.0002),
            enhance_threshold_factor: Some((0.0002,1.5)),
            multiply_factor: Some(1.1)
        }; 3], NX, alpha, gamma);
        let mut output = vec![0.0; NX];
        pyramid.apply_to_slice(&Y, output.as_mut_slice());
        println!("{Y:?}\n");
        println!("{output:?}");
    }
}
