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

use num::{Integer, integer::binomial};

use crate::pulse_detection::window::SliceWindow;

use super::{Real, Window};
use std::collections::VecDeque;

/// Specifies a kernel that resolves to a `Vec<Real>` by calling `Self::generate_kernel`.
#[derive(Clone)]
pub(crate) enum KernelType {
    Gaussian { sigma: Real },
    FiniteDifference { order: usize },
    Composition { left: Box<KernelType>, right: Box<KernelType> },
}

impl Default for KernelType {
    fn default() -> Self {
        Self::Gaussian { sigma: 0.0 }
    }
}

impl KernelType {
    fn generate_kernel(self) -> Vec<Real> {
        match self {
            KernelType::Gaussian { sigma } => {
                if sigma <= 0.0 {
                    return vec![1.0];
                }
                let s2 = sigma * sigma;
                let radius = i32::max(1, Real::ceil(4.0 * sigma) as i32);

                let size = 2 * radius as usize + 1;
                let mut kernel = (0..size)
                    .map(|i| i as Real - radius as Real)
                    .map(|x| Real::exp(-0.5 * x.powi(2) / s2))
                    .collect::<Vec<_>>();

                let kernel_sum = kernel.iter().sum::<Real>();
                kernel.iter_mut().for_each(|v| {
                    *v /= kernel_sum;
                });
                kernel
            }
            KernelType::FiniteDifference { order } => {
                (0..order + 1)
                    .map(|i| if (i + order).is_even() { 1. } else { -1. } * (binomial(order, i) as Real))
                    .collect::<Vec<_>>()
            }
            KernelType::Composition { left, right } => {
                let left = left.generate_kernel();
                let right = right.generate_kernel();
                (0..left.len() + right.len()).map(|i|
                    (0..right.len())
                        .map(|j| (i < left.len() + j && i >= j)
                            .then(||left[i - j]*right[j])
                            .unwrap_or_default()
                        ).sum()
                ).collect()
            }
        }
    }
}

#[derive(Default, Clone)]
pub(crate) struct ConvolutionFilter {
    value: Real,
    sqrt_kernel_sum_of_squares: Real,
    size: Real,
    kernel: Vec<Real>,
    window: VecDeque<Real>,
}

impl ConvolutionFilter {
    pub(crate) fn new(kernel_type: KernelType) -> Self {
        let kernel = kernel_type.generate_kernel();
        let sqrt_kernel_sum_of_squares = kernel.iter().map(|v| v.powi(2)).sum::<Real>().sqrt();
        let size = kernel.len() as Real;
        ConvolutionFilter {
            window: VecDeque::<Real>::with_capacity(kernel.len()),
            kernel,
            size,
            sqrt_kernel_sum_of_squares,
            ..Default::default()
        }
    }

    fn is_full(&self) -> bool {
        self.window.len() == self.size as usize
    }

    pub(crate) fn sqrt_kernel_sum_of_abs_squares(&self) -> Real {
        self.sqrt_kernel_sum_of_squares
    }

    pub(crate) fn kernel_size(&self) -> usize {
        self.kernel.len()
    }

    pub(crate) fn apply_slice(&self, slice: &[Real]) -> Real {
        let mut sum = 0.0;
        for i in 0..self.kernel.len() {
            sum += self.kernel[i]*slice[i];
        }
        sum
    }
}

impl Window for ConvolutionFilter {
    type TimeType = Real;
    type InputType = Real;
    type OutputType = Real;

    fn push(&mut self, value: Real) -> bool {
        if self.is_full() {
            self.window.pop_front().unwrap_or_default();
        }
        self.window.push_back(value);
        self.value = Iterator::zip(self.kernel.iter(), self.window.iter())
            .map(|(x, y)| x * y)
            .sum();
        self.is_full()
    }

    fn output(&self) -> Option<Real> {
        if self.is_full() {
            Some(self.value)
        } else {
            None
        }
    }

    fn apply_time_shift(&self, time: Real) -> Real {
        time - (self.size - 1.) / 2.0
    }
}
impl SliceWindow for ConvolutionFilter {
    type TimeType = Real;
    type InputType = Real;
    type OutputType = Real;
/*
    fn apply_to_slice<'a>(&self, output: &'a mut[Self::InputType]) -> &'a [Self::InputType] {
        let output_range = 0..output.len() - self.kernel_size() + 1;
        for i in output_range.clone() {
            let value = self.apply_slice(&output[i..i + self.kernel_size()]);
            output[i] = value;
        }
        &output[output_range]
    }
 */
    fn apply_to_slice<'a>(&self, input: &'a [Self::InputType], output: &'a mut[Self::OutputType]) {
        for i in 0..output.len() {
            let value = self.apply_slice(&input[i..i + self.kernel_size()]);
            output[i] = value;
        }
    }

    fn apply_time_shift(&self, time: Self::TimeType) -> Self::TimeType {
        time - (self.size - 1.) / 2.0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse_detection::iterators::{PaddingIterable, WindowIterable};
    use assert_approx_eq::assert_approx_eq;
    use digital_muon_common::Intensity;

    #[test]
    fn test_gaussian_kernel_sigma_zero() {
        let filter = ConvolutionFilter::new(KernelType::Gaussian { sigma: 0.0 });
        assert_eq!(filter.size, 1.0);
        assert_eq!(filter.kernel.len(), 1);
        assert_eq!(filter.window.capacity(), 1);
        // Test Kernel Integrates to One
        assert_approx_eq!(filter.kernel.iter().sum::<Real>(), 1.0);
        // Test Symmetry
        assert_eq!(
            filter.kernel,
            filter.kernel.iter().cloned().rev().collect::<Vec<_>>()
        );
        // Test Left-Most Values
        assert_eq!(filter.kernel[0..(filter.size as usize / 2 + 1)], vec![1.0]);
    }

    #[test]
    fn test_gaussian_kernel_sigma_one_eighth() {
        let filter = ConvolutionFilter::new(KernelType::Gaussian { sigma: 0.125 });
        assert_eq!(filter.size, 3.0);
        assert_eq!(filter.kernel.len(), 3);
        assert_eq!(filter.window.capacity(), 3);
        // Test Kernel Integrates to One
        assert_approx_eq!(filter.kernel.iter().sum::<Real>(), 1.0);
        // Test Symmetry
        assert_eq!(
            filter.kernel,
            filter.kernel.iter().cloned().rev().collect::<Vec<_>>()
        );
        // Test Left-Most Values
        assert_eq!(
            filter.kernel[0..(filter.size as usize / 2 + 1)],
            vec![1.2664165549093855e-14, 0.9999999999999747]
        );
    }

    #[test]
    fn test_gaussian_kernel_sigma_one() {
        let filter = ConvolutionFilter::new(KernelType::Gaussian { sigma: 1.0 });
        assert_eq!(filter.size, 9.0);
        assert_eq!(filter.kernel.len(), 9);
        assert_eq!(filter.window.capacity(), 9);
        // Test Kernel Integrates to One
        assert_approx_eq!(filter.kernel.iter().sum::<Real>(), 1.0);
        // Test Symmetry
        assert_eq!(
            filter.kernel,
            filter.kernel.iter().cloned().rev().collect::<Vec<_>>()
        );
        // Test Left-Most Values
        assert_eq!(
            filter.kernel[0..(filter.size as usize / 2 + 1)],
            vec![
                0.00013383062461474175,
                0.0044318616200312655,
                0.05399112742070441,
                0.24197144565660073,
                0.39894346935609776
            ]
        );
    }

    #[test]
    fn test_gaussian_kernel_sigma_two() {
        let filter = ConvolutionFilter::new(KernelType::Gaussian { sigma: 2.0 });
        assert_eq!(filter.size, 17.0);
        assert_eq!(filter.kernel.len(), 17);
        assert_eq!(filter.window.capacity(), 17);
        // Test Kernel Integrates to One
        assert_approx_eq!(filter.kernel.iter().sum::<Real>(), 1.0);
        // Test Symmetry
        assert_eq!(
            filter.kernel,
            filter.kernel.iter().cloned().rev().collect::<Vec<_>>()
        );
        // Test Left-Most Values
        assert_eq!(
            filter.kernel[0..(filter.size as usize / 2 + 1)],
            vec![
                6.691628957263553e-5,
                0.0004363490205067883,
                0.002215963172596555,
                0.00876430436278587,
                0.026995957967298846,
                0.06475993660472744,
                0.12098748976534904,
                0.17603575888479037,
                0.199474647864745
            ]
        );
    }

    #[test]
    fn test_window_kernel_two() {
        let data = [
            4.0, 3.0, 2.0, 5.0, 6.0, 1.0, 5.0, 7.0, 2.0, 4.0, 4.0, 3.0, 2.0, 5.0, 6.0, 1.0, 5.0,
            7.0, 2.0, 4.0,
        ];
        assert!(
            data.into_iter()
                .enumerate()
                .map(|(i, v)| (i as Real, v as Real))
                .window(ConvolutionFilter::new(KernelType::Gaussian { sigma: 2.0 }))
                .next()
                .is_some()
        );
    }

    #[test]
    fn test_no_data() {
        let data: [Real; 0] = [];
        assert!(
            data.into_iter()
                .enumerate()
                .map(|(i, v)| (i as Real, v as Real))
                .window(ConvolutionFilter::new(KernelType::Gaussian { sigma: 2.0 }))
                .next()
                .is_none()
        );
    }
    #[test]
    fn test_insufficient_data() {
        let data = [4.0, 3.0];
        assert!(
            data.into_iter()
                .enumerate()
                .map(|(i, v)| (i as Real, v as Real))
                .window(ConvolutionFilter::new(KernelType::Gaussian { sigma: 2.0 }))
                .next()
                .is_none()
        );
    }

    #[test]
    fn test_minimal() {
        let data = [
            4.0, 3.0, 2.0, 4.0, 3.0, 2.0, 4.0, 3.0, 2.0, 4.0, 3.0, 2.0, 4.0, 3.0, 2.0, 4.0, 3.0,
        ];
        let (i, value) = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(ConvolutionFilter::new(KernelType::Gaussian { sigma: 2.0 }))
            .next()
            .unwrap();
        assert_eq!(i, 8.0);
        assert_approx_eq!(value, 2.99986);
    }

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

    #[test]
    fn test_gaussian_convolution() {
        let gaussian_filter = ConvolutionFilter::new(KernelType::Gaussian { sigma: 2.0 });
        let y_iter = Y.into_iter();
        let padded_iter = y_iter
            .clone()
            .pad_reflect(
                gaussian_filter.kernel_size() / 2,
                gaussian_filter.kernel_size() / 2,
            )
            .enumerate()
            .map(|(i, v)| (i as Real, v));

        let smooth = padded_iter.window(gaussian_filter).collect::<Vec<_>>();

        const SMOOTH: [f64; NX] = [
            0.031268130092906694,
            0.033234761778690815,
            0.0381495589467257,
            0.04728157508788843,
            0.06099408488386007,
            0.07776935195174116,
            0.09437286547878088,
            0.10727019110556643,
            0.1142630852761387,
            0.1151547967885086,
            0.1112361176361001,
            0.10430856942280944,
            0.09597203881078058,
            0.08735992203314143,
            0.07913476898999633,
            0.07157093538873151,
            0.06470631547200212,
            0.05854161039310258,
            0.05313606679599597,
            0.04850117590953095,
            0.04446755377388985,
            0.040779531694687385,
            0.0373551060724061,
            0.03437512246778705,
            0.032052822867821185,
            0.030332730799984026,
            0.028855154849613006,
            0.027233176795293888,
            0.025410381383197424,
            0.02384355350198609,
            0.023411538190055627,
            0.025127639165856586,
            0.029801946195628218,
            0.03773283668763272,
            0.048445553164659975,
            0.06059992433844775,
            0.07229384689429848,
            0.081741672553697,
            0.08786117577604811,
            0.09033011460842023,
            0.08927088208659109,
            0.08507776779624915,
            0.07852386990120933,
            0.07078407189731849,
            0.06309586025394033,
            0.05626032586587045,
            0.05041436065089525,
            0.04524653984230089,
            0.04042598051031978,
            0.035879410762724814,
            0.03174947693107391,
            0.0281767258653733,
            0.025176018409067632,
            0.0227085645433209,
            0.020789217541235038,
            0.019459616349756705,
            0.01868113515083695,
            0.01830746793794243,
            0.01815769178915403,
            0.018088418068621964,
            0.018005934280877125,
            0.01785355793060935,
            0.017623201896661358,
            0.01737223474235957,
            0.017175390262333008,
            0.01702116978697179,
            0.016794353781839393,
            0.016420226967156455,
            0.016004289484102117,
            0.015754776789370938,
            0.0157510470132981,
            0.015846602682627012,
            0.015848057142197617,
            0.01575950782779819,
            0.01578730056516899,
            0.016099867159238425,
            0.016629713138296982,
            0.01711720706389652,
            0.017297327343169425,
            0.017034710983382318,
            0.01634866316947405,
            0.015377379800403575,
            0.01432472240477469,
            0.013420742090395526,
            0.012891545735861019,
        ];
        assert_eq!(SMOOTH.len(), smooth.len());

        for ((_, y1), y2) in smooth.iter().zip(SMOOTH.iter()) {
            assert_eq!(y1, y2);
        }
    }

    #[test]
    fn test_finite_differnece() {
        let kernel = KernelType::FiniteDifference { order: 1 };
        assert_eq!(kernel.generate_kernel(), [-1.0, 1.0]);
        let kernel = KernelType::FiniteDifference { order: 2 };
        assert_eq!(kernel.generate_kernel(), [1.0, -2.0, 1.0]);
        let kernel = KernelType::FiniteDifference { order: 3 };
        assert_eq!(kernel.generate_kernel(), [-1.0, 3.0, -3.0, 1.0]);
        let kernel = KernelType::FiniteDifference { order: 4 };
        assert_eq!(kernel.generate_kernel(), [1.0, -4.0, 6.0, -4.0, 1.0]);
    }

    #[test]
    fn test_first_finite_differnece_sample_data() {
        let input: Vec<Intensity> = vec![0, 6, 2, 1, 3, 1, 0];
        let kernel = KernelType::FiniteDifference { order: 1 };
        let conv = ConvolutionFilter::new(kernel);
        assert_eq!(conv.kernel_size(), 2);

        let slice_input = input.iter().cloned().map(|x|x as Real).collect::<Vec<_>>();
        let mut slice_output = vec![0.0; 6];
        conv.apply_to_slice(slice_input.as_slice(), slice_output.as_mut_slice());

        let mut output = input
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(conv);


        let expected = [6., -4., -1., 2., -2., -1.];
        for i in 0..6{
            let next = output.next();
            assert_eq!(next, Some((i as Real + 0.5, expected[i])));
            assert_eq!(expected[i], slice_output[i]);
        }
        assert!(output.next().is_none());
    }

    #[test]
    fn test_second_finite_differnece_sample_data() {
        let input: Vec<Intensity> = vec![0, 6, 2, 1, 3, 1, 0];

        let kernel = KernelType::FiniteDifference { order: 2 };
        let conv = ConvolutionFilter::new(kernel);
        assert_eq!(conv.kernel_size(), 3);

        let slice_input = input.iter().cloned().map(|x|x as Real).collect::<Vec<_>>();
        let mut slice_output = vec![0.0; 5];
        conv.apply_to_slice(slice_input.as_slice(), &mut slice_output);

        let mut output = input
            .iter()
            .enumerate()
            .map(|(i, v)| (i as Real, *v as Real))
            .window(conv);

        let expected = [-10., 3., 3., -4., 1.];
        for i in 0..5 {
            assert_eq!(output.next(), Some((i as Real + 1.0, expected[i])));
            assert_eq!(expected[i], slice_output[i]);
        }
        assert!(output.next().is_none());
    }

    #[test]
    fn test_convolution_composition_commutativity() {
        let kernel_1 = KernelType::Gaussian { sigma: 2.0 };
        let kernel_2 = KernelType::FiniteDifference { order: 2 };
        let kernel_12 = KernelType::Composition { left: Box::new(kernel_1.clone()), right: Box::new(kernel_2.clone()) };
        let kernel_21 = KernelType::Composition { left: Box::new(kernel_2.clone()), right: Box::new(kernel_1.clone()) };
        let conv_12 = ConvolutionFilter::new(kernel_12);
        let conv_21 = ConvolutionFilter::new(kernel_21);
        
        let sum_of_sizes = kernel_1.generate_kernel().len() + kernel_2.generate_kernel().len();
        assert_eq!(conv_12.kernel_size(), sum_of_sizes);
        assert_eq!(conv_21.kernel_size(), sum_of_sizes);
        for (a,b) in Iterator::zip(conv_12.kernel.into_iter(),conv_21.kernel.into_iter()) {
            assert_approx_eq!(a,b);
        }
    }
}
