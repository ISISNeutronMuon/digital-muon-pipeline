use num::Integer;
use rustfft::{
    FftPlanner,
    num_complex::{Complex, ComplexFloat},
};

use crate::pulse_detection::{
    Real,
    iterators::ZeroPaddingIterable,
    window::SliceWindow,
};

#[derive(Default, Clone)]
pub(crate) struct FftInverse<FT> {
    padded_vector_size: usize,
    truncation_size: usize,
    support: Vec<i32>,
    transform: FT,
}

impl<FT> FftInverse<FT> {
    pub(crate) fn new(
        padded_vector_size: usize,
        truncation_size: usize,
        support: Vec<i32>,
        transform: FT,
    ) -> Self {
        FftInverse {
            padded_vector_size,
            truncation_size,
            support,
            transform,
        }
    }

    fn get_even_mask(&self, input: &[Real]) -> Vec<Real> {
        let even_mask = self
            .support
            .iter()
            .enumerate()
            .filter_map(|(index, support)| {
                support.is_even().then_some(index).map(|index| {
                    input
                        .get(index)
                        .expect("input should have corresponding index, this should never fail")
                })
            })
            .cloned()
            .collect::<Vec<_>>();

        if (even_mask.iter().sum::<Real>() - 1.0).abs() > Real::EPSILON {
            panic!(
                "The even mask does not add to 1! {}",
                even_mask.iter().sum::<Real>() - 1.0
            );
        }
        even_mask
    }

    fn padded_complex_from_reals(input: Vec<Real>, padding_size: usize) -> Vec<Complex<Real>> {
        input
            .into_iter()
            .pad_zeroes(padding_size, padding_size)
            .map(|real| rustfft::num_complex::Complex::new(real, Default::default()))
            .collect::<Vec<_>>()
    }

    fn iter_about_max(input: Vec<Real>, radius: usize) -> impl Iterator<Item = Real> + Clone {
        //  Find the index of the max value of the resulting buffer.
        let arg_max = input
            .iter()
            .enumerate()
            .max_by(|r1, r2| {
                Real::partial_cmp(r1.1, r2.1).expect("Numbers are finite, this should never fail.")
            })
            .map(|r| r.0)
            .expect("Vector should be nonempty, this should never fail.");

        // Truncate the buffer about the `arg_max` element.
        input
            .into_iter()
            .take(arg_max + radius + 1)
            .skip(arg_max - radius + 1)
    }
}

impl<FT> SliceWindow for FftInverse<FT>
where
    FT: Fn(Complex<Real>) -> Complex<Real> + Clone,
{
    type TimeType = Real;
    type InputType = Real;
    type OutputType = Real;

    fn apply_to_slice<'a>(&self, input: &'a [Self::InputType], output: &'a mut [Self::OutputType]) {
        // Mask out the noneven values of support of input
        let even_mask = self.get_even_mask(input);

        // Pad even mask with the appropriate number of zeroes.
        let padding_size = (self.padded_vector_size / 2) - even_mask.len().div_ceil(2);
        let mut padded_even_mask = Self::padded_complex_from_reals(even_mask, padding_size);

        // Create an `FftPlanner`.
        let mut fft_planner = FftPlanner::new();

        // We perform the FFT on `padded_even_mask` and take the recipricol of each element.
        let fft = fft_planner.plan_fft_forward(padded_even_mask.len());
        fft.process(&mut padded_even_mask);
        let mut padded_even_mask_recip = padded_even_mask
            .into_iter()
            .map(self.transform.clone())
            .collect::<Vec<_>>();

        // We perform the inverse FFT on `padded_even_mask_recip`, and take the real part.
        let ifft = fft_planner.plan_fft_inverse(padded_even_mask_recip.len());
        ifft.process(&mut padded_even_mask_recip);
        let padded_even_mask_recip_real = padded_even_mask_recip
            .into_iter()
            .map(Complex::re)
            .collect::<Vec<_>>();

        // Truncate the buffer about the `arg_max` element.
        let mut iter =
            Self::iter_about_max(padded_even_mask_recip_real, self.truncation_size / 2 + 1);

        // Normalise and return.
        let sum = iter.clone().sum::<Real>();
        for out in output {
            *out = iter
                .next()
                .expect("Iterator should have sufficient values, this should never fail.")
                / sum;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse_detection::window::SliceWindow;
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn test_invertability() {
        let alpha = vec![0.125, 0.175, 0.4, 0.175, 0.125];
        let support = vec![0, 0, 0, 0, 0];
        let mut gamma = vec![0.0; 5];
        let fft = FftInverse::new(8, 5, support.clone(), |x| x);
        fft.apply_to_slice(alpha.as_slice(), gamma.as_mut_slice());
        let fft = FftInverse::new(8, 5, vec![0, 0, 0, 0, 0], |x| x);
        let mut alpha2 = vec![0.0; 5];
        fft.apply_to_slice(gamma.as_slice(), alpha2.as_mut_slice());
        for (i, alpha2) in alpha2.into_iter().enumerate() {
            assert_approx_eq!(alpha[i], alpha2);
        }
    }

    #[test]
    fn test_reverse() {
        let alpha = vec![0.125, 0.5, 0.75, 0.5, 0.125];
        let support = vec![-2, -1, 0, 1, 2];
        let mut gamma = vec![0.0; 5];
        let fft = FftInverse::new(200, 6, support.clone(), Complex::recip);
        fft.apply_to_slice(alpha.as_slice(), gamma.as_mut_slice());
    }
}
