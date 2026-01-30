//! This detector registers an event whenever the derivative of the input stream [FIXME]
//! This algorithm uses: Richard Waite's second derivative smoothing peak finder,
//! translated from C++ by Boris Shustin and Jaroslav Fowlkes for ALC, and translated
//! thereafter into Rust.

use std::usize;

use crate::pulse_detection::Real;

/// Second derivative smoothing peak finder
/// # Parameters
/// - x: x data.
/// - y: y data.
/// - noise_centile: centile of x to use for noise estimation.
/// - kernel_sigma: sigma of the Gaussian kernel for smoothing.
/// - nsig_noise: number of standard deviations above noise to use as threshold.
/// - min_size: minimum size of region to consider a peak, if negative all regions are considered (use negative value to consider all regions).
///
/// # Return
/// (x_peaks, y_peaks) - 1D arrays of peak locations
///
pub(crate) fn sec_deriv_smoothing_for_peaks(
    x: &[Real],
    y: &[Real],
    noise_centile: Real,
    kernel_sigma: Real,
    nsig_noise: Real,
    min_size: Option<usize>,
) -> Result<(Vec<Real>, Vec<Real>), String> {
    if x.len() != y.len() {
        return Err("x and y must have same length".into());
    }
    let n = x.len();
    if n == 0 {
        return Ok(Default::default());
    }

    // 1. Smooth y with Gaussian kernel
    let kernel = gaussian_kernel(kernel_sigma);
    let y_smooth = convolve_reflect(y, &kernel);

    // 2. second derivative (discrete Laplacian in 1D): d2[i] = y_smooth[i-1] - 2*y_smooth[i] + y_smooth[i+1]
    // use reflect for boundaries
    let yd2: Vec<f64> = (0..n)
        .map(|i| {
            let im = reflect_index(i as i32 - 1, n);
            let ip = reflect_index(i as i32 + 1, n);
            y_smooth[im] - 2.0 * y_smooth[i] + y_smooth[ip]
        })
        .collect::<Vec<_>>();

    // 3. estimate noise from last portion of x (x > percentile(x, noise_centile))
    //let x_percentile = percentile(&x, noise_centile)?;
    /*let noise_samples = Iterator::zip(x.iter(), yd2.iter())
        .filter_map(|(&x, &yd2)| (x > x_percentile).then_some(yd2))
        .collect::<Vec<_>>();*/
    let noise_std = stddev(&yd2[0..((yd2.len() as f64*noise_centile)/100.0) as usize]);

    // 4. label contiguous regions where yd2 < -nsig_noise * noise_std
    let threshold = -nsig_noise * noise_std;
    //let mut current_label = 0;
    //let mut prev_label = 0;
    let regions = {
        match yd2.iter()
            .enumerate()
            .fold((Vec::<(usize,usize)>::new(), None::<(usize,usize)>), |(mut acc, existing), (i, &yd2)| {
                match existing {
                    Some((start,end)) => {
                        if yd2 < threshold {
                            (acc, Some((start, i)))
                        } else {
                            acc.push((start,end));
                            (acc, None)
                        }
                    },
                    None => {
                        if yd2 < threshold {
                            (acc, Some((i, i)))
                        } else {
                            (acc, None)
                        }
                    },
                }
            }) {
                (mut acc, Some(next)) => {
                    acc.push(next);
                    acc
                },
                (acc, None) => acc
            }
        };

    /*let filtered = yd2.iter().enumerate().filter(|&(_, &yd2)|yd2 < threshold).collect::<Vec<_>>();
    let labels = (0..n)
        .map(|i| {
            let this_label = {
                if yd2[i] < threshold {
                    if i == 0 || prev_label == 0 {
                        current_label += 1;
                    }
                    current_label
                } else {
                    usize::default()
                }
            };
            prev_label = this_label;
            this_label
        })
        .collect::<Vec<_>>();
    let nlabels = current_label;*/

    // collect slices (start, end inclusive) for each label
    /*let mut slices = vec![None; regions.len()];
    if regions.len() > 0 {
        for (i, label) in regions
            .into_iter()
            .enumerate()
            .filter(|&(_, label)| label > 0)
        {
            *slices.get_mut(label - 1).expect("") = Some((i, i));
        }
    }*/

    // 5. pick peaks
    // indices of peaks
    let ipks = match min_size {
        Some(min_size) => {
            // Filter labeled regions where region length > min_size,
            regions
                .iter()
                .filter(|&(start, end)| end - start + 1 > min_size)
                .map(|&(start, end)| {
                    let length = end - start + 1;
                    let segment = yd2
                        .iter()
                        .skip(start)
                        .take(length)
                        .copied()
                        .collect::<Vec<Real>>();

                    let relmin = argrelmin_segment(&segment);
                    if relmin.is_empty() {
                        vec![start] // fallback to first element in region
                    } else {
                        relmin.iter().map(|&r| start + r).collect()
                    }
                })
                .flatten()
                .collect()
        }
        None => {
            // For each labeled region, take the index of the global minimum (argmin of yd2) within the region
            regions
                .iter()
                .map(|(start, end)| {
                    yd2.iter()
                        .enumerate()
                        .take(*end + 1)
                        .skip(*start)
                        .fold(
                            (*start, yd2[*start]),
                            |(best_i, best_v), (new_i, &new_v)| {
                                if new_v < best_v {
                                    (new_i, new_v)
                                } else {
                                    (best_i, best_v)
                                }
                            },
                        )
                        .0
                })
                .collect::<Vec<_>>()
        }
    };

    // Produce x and y arrays at peak indices
    let mut xpk = Vec::<Real>::with_capacity(ipks.len());
    let mut ypk = Vec::<Real>::with_capacity(ipks.len());
    for idx in ipks {
        if idx < n {
            xpk.push(x[idx]);
            ypk.push(y[idx]);
        }
    }
    return Ok((xpk, ypk));
}

// Compute Gaussian kernel
fn gaussian_kernel(sigma: Real) -> Vec<Real> {
    if sigma <= 0.0 {
        return vec![1.0];
    }
    let s2 = sigma * sigma;
    let radius = i32::max(1, Real::ceil(3.0 * sigma) as i32);

    let size = 2 * radius as usize + 1;
    let mut kernel = (0..size)
        .map(|i| i as Real - radius as Real)
        .map(|x| Real::exp(-0.5 * x.powi(2) / s2))
        .collect::<Vec<_>>();

    let kernel_sum = kernel.iter().sum::<Real>();
    kernel.iter_mut().for_each(|v| {
        *v /= kernel_sum;
    });
    return kernel;
}

// function to reflect an index
fn reflect_index(idx: i32, n: usize) -> usize {
    if n == 0 {
        return 0;
    } else if idx < 0 {
        return (-idx - 1) as usize;
    } else if idx >= n as i32 {
        return 2 * n - idx as usize - 1;
    }
    return idx as usize;
}

// Gaussian Laplace filter
fn convolve_reflect(data: &[Real], kernel: &[Real]) -> Vec<Real> {
    let data_length = data.len();
    let radius = kernel.len() as i32 / 2;
    (0..data_length as i32)
        .map(|idx| {
            kernel
                .iter()
                .enumerate()
                .map(|(kernel_idx, &coef)| coef * data[reflect_index(idx + (kernel_idx as i32 - radius), data_length)])
                .sum()
        })
        .collect()
}

// Compute percentile
fn percentile(v: &[Real], p: Real) -> Result<Real, String> {
    if v.is_empty() {
        return Err("percentile: empty input".into());
    }
    let real_cmp = |a: &Real, b: &Real| {
        a.partial_cmp(b)
            .expect("Values are numbers, this should never fail")
    };
    if p <= 0.0 {
        return Ok(v
            .iter()
            .copied()
            .min_by(real_cmp)
            .expect("Min exists, this should never fail"));
    }
    if p >= 100.0 {
        return Ok(v
            .iter()
            .copied()
            .max_by(real_cmp)
            .expect("Max exists, this should never fail"));
    }
    let mut tmp = v.to_vec();
    tmp.sort_by(real_cmp);
    let pos = (p / 100.0) * (tmp.len() - 1) as Real;
    let floor = pos.floor();
    let ceil = pos.ceil();
    if floor == ceil {
        return Ok(tmp[floor as usize]);
    }
    let frac = pos - floor;
    return Ok(tmp[floor as usize] * (1.0 - frac) + tmp[ceil as usize] * frac);
}

// Compute standard deviation
fn stddev(v: &[Real]) -> Real {
    if v.is_empty() {
        return 0.0;
    }
    let mean: Real = v.iter().sum::<Real>() / v.len() as Real;
    v.iter()
        .map(|&x: &Real| (x - mean).powi(2))
        .sum::<Real>()
        .sqrt()
        / v.len() as Real // FIXME: Should this not be divide by n - 1?
}

/// find indices of relative minima in a vector segment [0..n-1] (returns indices relative to segment start)
fn argrelmin_segment(seg: &[Real]) -> Vec<usize> {
    (1..(seg.len() - 1))
        .filter(|&i| seg[i] < seg[i - 1] && seg[i] < seg[i + 1])
        .collect()
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;
    use super::*;

    // number of data points
    const NX: usize = 85;


    // data x values
    const X : [f64; NX] = [
        3.6048,
        3.6056000000000004,
        3.6064000000000003,
        3.6071999999999997,
        3.608,
        3.6088,
        3.6096,
        3.6104000000000003,
        3.6112,
        3.612,
        3.6128,
        3.6136,
        3.6144000000000003,
        3.6152,
        3.616,
        3.6168000000000005,
        3.6176000000000004,
        3.6184,
        3.6192,
        3.62,
        3.6208,
        3.6216000000000004,
        3.6224000000000003,
        3.6232,
        3.624,
        3.6248,
        3.6256,
        3.6264000000000003,
        3.6272,
        3.628,
        3.6288000000000005,
        3.6296,
        3.6304,
        3.6312,
        3.632,
        3.6328,
        3.6336000000000004,
        3.6344000000000003,
        3.6351999999999998,
        3.636,
        3.6368,
        3.6376,
        3.6384000000000003,
        3.6392,
        3.64,
        3.6408,
        3.6416,
        3.6424,
        3.6432,
        3.644,
        3.6448,
        3.6456000000000004,
        3.6464,
        3.6471999999999998,
        3.648,
        3.6488,
        3.6496,
        3.6504000000000003,
        3.6512000000000002,
        3.652,
        3.6528,
        3.6536,
        3.6544000000000003,
        3.6552000000000002,
        3.656,
        3.6568000000000005,
        3.6576,
        3.6584,
        3.6592000000000002,
        3.66,
        3.6608,
        3.6616000000000004,
        3.6624000000000003,
        3.6632,
        3.664,
        3.6648,
        3.6656,
        3.6664000000000003,
        3.6672000000000002,
        3.668,
        3.6688,
        3.6696,
        3.6704,
        3.6712000000000002,
        3.672,
    ];

    // data y values
    const Y : [f64; NX] = [
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
    fn test_positive_threshold() {
        let (x,y) = sec_deriv_smoothing_for_peaks(&X, &Y, 90.0, 2.0, 5.0, Some(2)).unwrap();
        assert_eq!(x.len(), 2);
        assert_eq!(y.len(), 2);
        assert_approx_eq!(x[0], 3.6112);
        assert_approx_eq!(y[0], 0.1283464566929134);
        assert_approx_eq!(x[1], 3.6352);
        assert_approx_eq!(y[1], 0.09291338582677167);
    }
}
