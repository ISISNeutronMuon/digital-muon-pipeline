use num::Integer;
use rustfft::{
    FftPlanner,
    num_complex::{Complex, ComplexFloat},
};

use crate::pulse_detection::{Real, iterators::ZeroPaddingIterable, window::SliceWindow};

fn find_arg_max(input: &[Real]) -> usize {
    //  Find the index of the max value of the resulting buffer.
    input
        .iter()
        .enumerate()
        .max_by(|r1, r2| {
            r1.1.partial_cmp(r2.1)
                .expect("Numbers are finite, this should never fail.")
        })
        .expect("Vector should be nonempty, this should never fail.")
        .0
}

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
        let padding_size = ((self.padded_vector_size as Real / 2.0)
            - even_mask.len().div_ceil(2) as Real) as usize;
        let mut padded_even_mask = even_mask
            .into_iter()
            .pad_zeroes(padding_size, padding_size)
            .map(|real| rustfft::num_complex::Complex::new(real, Default::default()))
            .collect::<Vec<_>>();

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

        // Truncate the buffer about the maximum element.
        // FIXME: This needs guards for reading invalid indices.
        let arg_max = find_arg_max(&padded_even_mask_recip_real);
        let slice = &padded_even_mask_recip_real
            [(arg_max - self.truncation_size / 2)..(arg_max + self.truncation_size / 2 + 1)];

        // Write normalised values to output.
        let sum = slice.iter().sum::<Real>();
        output
            .iter_mut()
            .zip(slice.iter())
            .for_each(|(out, val)| *out = val / sum)
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
    fn test_invertability_masked() {
        let input = vec![0.125, 0.5, 0.75, 0.5, 0.125];
        let support = vec![-2, -1, 0, 1, 2];
        let mut output = vec![0.0; 5];
        let fft = FftInverse::new(200, 5, support.clone(), |x| x);
        fft.apply_to_slice(input.as_slice(), output.as_mut_slice());
        const EXPECTED_OUTPUT: [Real; 5] = [0.0, 0.125, 0.75, 0.125, 0.0];
        for (i, out) in output.into_iter().enumerate() {
            assert_approx_eq!(EXPECTED_OUTPUT[i], out);
        }
    }

    #[test]
    fn test_reverse() {
        let input = vec![0.125, 0.5, 0.75, 0.5, 0.125];
        let support = vec![-2, -1, 0, 1, 2];
        let mut output = vec![0.0; 5];
        let fft = FftInverse::new(200, 5, support.clone(), Complex::recip);
        fft.apply_to_slice(input.as_slice(), output.as_mut_slice());
        const EXPECTED_OUTPUT: [Real; 5] =
            [0.04112906, -0.23971773, 1.39717735, -0.23971773, 0.04112906];
        for (i, out) in output.into_iter().enumerate() {
            assert_approx_eq!(EXPECTED_OUTPUT[i], out);
        }
    }
}
