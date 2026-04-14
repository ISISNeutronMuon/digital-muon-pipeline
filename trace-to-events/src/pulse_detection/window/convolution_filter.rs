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
    Gaussian {
        sigma: Real,
    },
    FiniteDifference {
        order: usize,
    },
    Composition {
        left: Box<KernelType>,
        right: Box<KernelType>,
    },
    ManualCoefficients(Vec<Real>),
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
            KernelType::FiniteDifference { order } => (0..order + 1)
                .map(
                    |i| if (i + order).is_even() { 1. } else { -1. } * (binomial(order, i) as Real),
                )
                .collect::<Vec<_>>(),
            KernelType::Composition { left, right } => {
                let left = left.generate_kernel();
                let right = right.generate_kernel();
                (0..left.len() + right.len())
                    .map(|i| {
                        (0..right.len())
                            .map(|j| {
                                if i < left.len() + j && i >= j {
                                    left[i - j] * right[j]
                                } else {
                                    Default::default()
                                }
                            })
                            .sum()
                    })
                    .collect()
            }
            KernelType::ManualCoefficients(coefs) => coefs,
        }
    }
}

#[derive(Default, Clone)]
pub(crate) struct ConvolutionFilter {
    value: Real,
    size: Real,
    kernel: Vec<Real>,
    window: VecDeque<Real>,
}

impl ConvolutionFilter {
    pub(crate) fn new(kernel_type: KernelType) -> Self {
        let kernel = kernel_type.generate_kernel();
        let size = kernel.len() as Real;
        ConvolutionFilter {
            window: VecDeque::<Real>::with_capacity(kernel.len()),
            kernel,
            size,
            ..Default::default()
        }
    }

    fn is_full(&self) -> bool {
        self.window.len() == self.size as usize
    }

    pub(crate) fn kernel_size(&self) -> usize {
        self.kernel.len()
    }

    pub(crate) fn apply_slice(&self, slice: &[Real]) -> Real {
        let mut sum = 0.0;
        for (i, value) in slice.iter().enumerate().take(self.kernel.len()) {
            sum += self.kernel[i] * value;
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

    fn apply_to_slice<'a>(&self, input: &'a [Self::InputType], output: &'a mut [Self::OutputType]) {
        for i in 0..output.len() {
            let value = self.apply_slice(&input[i..i + self.kernel_size()]);
            output[i] = value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pulse_detection::iterators::{PaddingIterable, WindowIterable},
        test_data::smoothing,
    };
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

    #[test]
    fn test_gaussian_convolution() {
        let gaussian_filter = ConvolutionFilter::new(KernelType::Gaussian { sigma: 2.0 });
        let y_iter = smoothing::VALUES.into_iter();
        let padded_iter = y_iter
            .clone()
            .pad_reflect(
                gaussian_filter.kernel_size() / 2,
                gaussian_filter.kernel_size() / 2,
            )
            .enumerate()
            .map(|(i, v)| (i as Real, v));

        let smooth = padded_iter.window(gaussian_filter).collect::<Vec<_>>();

        assert_eq!(smoothing::SMOOTHED_VALUED.len(), smooth.len());

        for ((_, y1), y2) in smooth.iter().zip(smoothing::SMOOTHED_VALUED.iter()) {
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

        let slice_input = input.iter().cloned().map(|x| x as Real).collect::<Vec<_>>();
        let mut slice_output = vec![0.0; 6];
        conv.apply_to_slice(slice_input.as_slice(), slice_output.as_mut_slice());

        let mut output = input
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(conv);

        let expected = [6., -4., -1., 2., -2., -1.];
        for i in 0..6 {
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

        let slice_input = input.iter().cloned().map(|x| x as Real).collect::<Vec<_>>();
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
        let kernel_12 = KernelType::Composition {
            left: Box::new(kernel_1.clone()),
            right: Box::new(kernel_2.clone()),
        };
        let kernel_21 = KernelType::Composition {
            left: Box::new(kernel_2.clone()),
            right: Box::new(kernel_1.clone()),
        };
        let conv_12 = ConvolutionFilter::new(kernel_12);
        let conv_21 = ConvolutionFilter::new(kernel_21);

        let sum_of_sizes = kernel_1.generate_kernel().len() + kernel_2.generate_kernel().len();
        assert_eq!(conv_12.kernel_size(), sum_of_sizes);
        assert_eq!(conv_21.kernel_size(), sum_of_sizes);
        for (a, b) in Iterator::zip(conv_12.kernel.into_iter(), conv_21.kernel.into_iter()) {
            assert_approx_eq!(a, b);
        }
    }
}
