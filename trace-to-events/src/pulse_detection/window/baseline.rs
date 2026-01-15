//! [TODO]
use super::{Real, Window};

#[derive(Default, Clone)]
pub(crate) struct Baseline {
    baseline: Real,
    value: Real,
    smoothing_factor: Real,
    warm_up: usize,
    time: usize,
}

impl Baseline {
    pub(crate) fn new(warm_up: usize, smoothing_factor: Real) -> Self {
        Baseline {
            warm_up,
            smoothing_factor,
            ..Default::default()
        }
    }
}

impl Window for Baseline {
    type TimeType = Real;
    type InputType = Real;
    type OutputType = Real;

    fn push(&mut self, value: Real) -> bool {
        self.value = value - self.baseline;
        if self.time < self.warm_up {
            self.baseline = if self.time == 0 {
                value
            } else {
                value * self.smoothing_factor + self.baseline * (1. - self.smoothing_factor)
            };
            self.time += 1;
            false
        } else {
            true
        }
    }

    fn output(&self) -> Option<Real> {
        (self.time == self.warm_up).then_some(self.value)
    }

    fn apply_time_shift(&self, time: Real) -> Real {
        time - (self.warm_up as Real)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse_detection::window::WindowFilter;
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn zero_baseline() {
        let input: Vec<Real> = vec![1.0, 3.0, 6.0, -1.0, 5.0];
        let output: Vec<_> = input
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(Baseline::new(0, 0.1))
            .collect();

        assert_eq!(output.len(), 5);
        assert_eq!(output[0], (0., 1.0));
        assert_eq!(output[1], (1., 3.0));
        assert_eq!(output[2], (2., 6.0));
        assert_eq!(output[3], (3., -1.0));
        assert_eq!(output[4], (4., 5.0));
    }

    #[test]
    fn constant_data() {
        let input: Vec<Real> = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let output: Vec<_> = input
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(Baseline::new(3, 0.1))
            .collect();

        assert_eq!(output.len(), 4);
        assert_eq!(output[0], (0., 0.));
        assert_eq!(output[1], (1., 0.));
        assert_eq!(output[2], (2., 0.));
        assert_eq!(output[3], (3., 0.));
    }

    #[test]
    fn initially_constant_data() {
        let input: Vec<Real> = vec![1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 3.0];
        let output: Vec<_> = input
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(Baseline::new(3, 0.1))
            //.map(|(_, x)| x)
            .collect();

        assert_eq!(output[0], (0., 0.));
        assert_eq!(output[1], (1., 0.));
        assert_eq!(output[2], (2., 1.));
        assert_eq!(output[3], (3., 2.));
    }

    #[test]
    fn varying_data() {
        let input: Vec<Real> = vec![1.0, 2.0, 0.0, 0.0, 1.0, 2.0, 3.0];
        let output: Vec<_> = input
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(Baseline::new(3, 0.2))
            .map(|(_, x)| x)
            .collect();

        assert_approx_eq!(output[0], -0.96, 1e-8);
        assert_approx_eq!(output[1], 0.04, 1e-8);
        assert_approx_eq!(output[2], 1.04, 1e-8);
        assert_approx_eq!(output[3], 2.04, 1e-8);
    }
}
