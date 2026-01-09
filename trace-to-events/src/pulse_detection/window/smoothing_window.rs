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
use super::{Real, Stats, Window};
use std::collections::VecDeque;

#[derive(Default, Clone)]
pub(crate) struct SmoothingWindow {
    value: Real,
    sum: Real,
    sum_of_squares: Real,
    size: Real,
    window: VecDeque<Real>,
}

impl SmoothingWindow {
    pub(crate) fn new(size: usize) -> Self {
        if size < 1 {
            panic!("Size must be >= 1");
        }
        SmoothingWindow {
            window: VecDeque::<Real>::with_capacity(size + 1),
            size: size as Real,
            ..Default::default()
        }
    }

    fn is_full(&self) -> bool {
        self.window.len() == self.size as usize
    }

    #[cfg(test)]
    fn test_mean(&self) -> Real {
        self.window.iter().sum::<f64>() / self.size
    }

    #[cfg(test)]
    fn test_variance(&self) -> Real {
        let mean = self.test_mean();
        self.window.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (self.size - 1.0)
    }
}

impl Window for SmoothingWindow {
    type TimeType = Real;
    type InputType = Real;
    type OutputType = Stats;

    fn push(&mut self, value: Real) -> bool {
        self.value = value;
        if self.size == 1.0 {
            return true;
        }
        if self.is_full() {
            let old = self.window.pop_front().unwrap_or_default();
            self.sum -= old;
            self.sum_of_squares -= old.powi(2);
        }
        self.sum += value;
        self.sum_of_squares += value.powi(2);
        self.window.push_back(value);
        self.is_full()
    }

    fn output(&self) -> Option<Stats> {
        if self.size == 1.0 {
            Some(Stats {
                value: self.value,
                mean: self.value,
                variance: 0.0,
            })
        } else if self.is_full() {
            Some(Stats {
                value: self.value,
                mean: self.sum / self.size,
                variance: (self.sum_of_squares - self.sum.powi(2) / self.size) / (self.size - 1.0),
            })
        } else {
            None
        }
    }

    fn apply_time_shift(&self, time: Real) -> Real {
        time - (self.size - 1.) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse_detection::window::WindowFilter;
    use assert_approx_eq::assert_approx_eq;

    #[test]
    #[should_panic]
    fn test_window_size_zero() {
        let data = [4.0, 3.0, 2.0, 5.0, 6.0, 1.0, 5.0, 7.0, 2.0, 4.0];
        data.into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(SmoothingWindow::new(0));
    }

    #[test]
    fn test_window_size_one() {
        let data = [4.0, 3.0, 2.0, 5.0];
        let iter = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(SmoothingWindow::new(1));

        assert!(iter.clone().all(|(_, stats)| stats.variance == 0.0));
        assert!(iter.clone().all(|(_, stats)| stats.value == stats.mean));
        let vals: Vec<_> = iter.map(|(_, stats)| stats.value).collect();
        assert_eq!(vals.len(), 4);
        assert_eq!(vals[0], 4.0);
        assert_eq!(vals[1], 3.0);
        assert_eq!(vals[2], 2.0);
        assert_eq!(vals[3], 5.0);
    }

    #[test]
    fn test_window_size_two() {
        let data = [4.0, 3.0, 2.0, 5.0, 6.0, 1.0, 5.0, 7.0, 2.0, 4.0];
        assert!(
            data.into_iter()
                .enumerate()
                .map(|(i, v)| (i as Real, v as Real))
                .window(SmoothingWindow::new(2))
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
                .window(SmoothingWindow::new(3))
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
                .window(SmoothingWindow::new(3))
                .next()
                .is_none()
        );
    }
    #[test]
    fn test_minimal() {
        let data = [4.0, 3.0];
        let (i, stats) = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(SmoothingWindow::new(2))
            .next()
            .unwrap();
        assert_eq!(i, 0.5);
        assert_eq!(stats.value, 3.);
        assert_approx_eq!(stats.mean, 7. / 2.);
        assert_approx_eq!(
            stats.variance,
            ((4. as Real - 7. / 2.).powi(2) + (3. as Real - 7. / 2.).powi(2)) / (2. - 1.)
        );
    }
    #[test]
    fn test_three_data() {
        let data = [4.0, 3.0, 1.0];
        let (i, stats) = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(SmoothingWindow::new(2))
            .nth(1)
            .unwrap();
        assert_eq!(i, 1.5);
        assert_eq!(stats.value, 1.);
        assert_approx_eq!(stats.mean, 2.);
        assert_approx_eq!(
            stats.variance,
            ((3. as Real - 2.).powi(2) + (1. as Real - 2.).powi(2)) / (2. - 1.)
        );
    }

    #[test]
    fn test_three_data_three_window() {
        let data = [4.0, 3.0, 1.0];
        let (i, stats) = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(SmoothingWindow::new(3))
            .next()
            .unwrap();
        assert_eq!(i, 1.);
        assert_eq!(stats.value, 1.);
        assert_approx_eq!(stats.mean, 8. / 3.);
        assert_approx_eq!(
            stats.variance,
            ((4. as Real - 8. / 3.).powi(2)
                + (3. as Real - 8. / 3.).powi(2)
                + (1. as Real - 8. / 3.).powi(2))
                / (3. - 1.)
        );
    }

    #[test]
    fn test_five_data_three_window() {
        let data = [4.0, 3.0, 1.0, 5.0, 3.0];
        let mut itr = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i as Real, v as Real))
            .window(SmoothingWindow::new(3));
        let (i, stats) = itr.next().unwrap();
        assert_eq!(i, 1.);
        assert_eq!(stats.value, 1.);
        assert_approx_eq!(stats.mean, 8. / 3.);
        assert_approx_eq!(
            stats.variance,
            ((4. as Real - 8. / 3.).powi(2)
                + (3. as Real - 8. / 3.).powi(2)
                + (1. as Real - 8. / 3.).powi(2))
                / (3. - 1.)
        );

        let (i, stats) = itr.next().unwrap();
        assert_eq!(i, 2.);
        assert_eq!(stats.value, 5.);
        assert_approx_eq!(stats.mean, 9. / 3.);
        assert_approx_eq!(
            stats.variance,
            ((3. as Real - 9. / 3.).powi(2)
                + (1. as Real - 9. / 3.).powi(2)
                + (5. as Real - 9. / 3.).powi(2))
                / (3. - 1.)
        );

        let (i, stats) = itr.next().unwrap();
        assert_eq!(i, 3.);
        assert_eq!(stats.value, 3.);
        assert_approx_eq!(stats.mean, 9. / 3.);
        assert_approx_eq!(
            stats.variance,
            ((1. as Real - 9. / 3.).powi(2)
                + (5. as Real - 9. / 3.).powi(2)
                + (3. as Real - 9. / 3.).powi(2))
                / (3. - 1.)
        );
    }

    #[test]
    fn test_variance_accuracy() {
        use rand::random;
        let data: Vec<Real> = (0..1000).map(|_| random()).collect();

        for window_size in 2..100 {
            let smoothing_window = SmoothingWindow::new(window_size);
            let mut itr = data
                .iter()
                .enumerate()
                .map(|(i, v)| (i as Real, *v as Real))
                .window(smoothing_window);
            while let Some(stat) = itr.next() {
                assert_approx_eq!(stat.1.mean, itr.get_window().test_mean());
                assert_approx_eq!(stat.1.variance, itr.get_window().test_variance());
            }
        }
    }

    #[test]
    fn test_variance_zero_on_constant_sequence() {
        use rand::random;
        let num = random::<Real>();
        let data: Vec<Real> = vec![num; 1000];

        for window_size in 2..100 {
            let smoothing_window = SmoothingWindow::new(window_size);
            let itr = data
                .iter()
                .enumerate()
                .map(|(i, v)| (i as Real, *v as Real))
                .window(smoothing_window);
            for stat in itr {
                assert_approx_eq!(stat.1.mean, num as Real, 1e-8);
                assert_approx_eq!(stat.1.variance, 0., 1e-8);
            }
        }
    }
}
