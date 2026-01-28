//! This detector registers an event whenever the derivative of the input stream [FIXME]
//!




use std::usize;

use crate::pulse_detection::Real;

/*
 * Richard Waite's second derivative smoothing peak finder (C++ translation)
 */


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
pub(crate) fn sec_deriv_smoothing_for_peaks(x : &[Real], y : &[Real], noise_centile : Real, kernel_sigma : Real, nsig_noise : Real, min_size : Option<usize> ) -> Result<(Vec<Real>, Vec<Real>),String> {
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
    let yd2 = (0..n).map(|i|{
        let im = reflect_index(i as i32 - 1, n);
        let ip = reflect_index(i as i32 + 1, n);
        y_smooth[im] - 2.0 * y_smooth[i] + y_smooth[ip]
    }).collect::<Vec<_>>();

    // 3. estimate noise from last portion of x (x > percentile(x, noise_centile))
    let xp = percentile(x, noise_centile)?;
    let noise_samples = x.iter()
        .zip(yd2.iter())
        .filter_map(|(&x, &yd2)|(x > xp).then_some(yd2))
        .collect::<Vec<_>>();

    let noise_std = stddev(&noise_samples);

    // 4. label contiguous regions where yd2 < -nsig_noise * noise_std
    let thresh = -nsig_noise * noise_std;
    let mut current_label = 0;
    let mut prev_label = 0;
    let labels = (0..n).map(|i| {
        let this_label = {
            if yd2[i] < thresh {
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
    }).collect::<Vec<_>>();
    let nlabels = current_label;

    // collect slices (start, end inclusive) for each label
    let mut slices = vec![None; nlabels];
    if nlabels > 0 {
        for (i,label) in labels.into_iter().enumerate().filter(|&(_, label)|label > 0) {
            *slices.get_mut(label - 1).expect("") = Some((i,i));
        }
    }
    /*let slices = {
        if nlabels > 0 {
            [(0.)].iter()
                .chain(labels.iter())
                .take(labels.len())
                .enumerate()
                .map(|(i, &label)|
                    (label > 0)
                        .then_some((i + 1, i + 1)))
                .collect()
        } else {
            Vec::<_>::default()
        }
    };*/

    // 5. pick peaks
    let mut ipks = Vec::<usize>::new(); // indices of peaks
    match min_size {
        Some(min_size) => {
            // For each labeled region (only if region length > min_size),
            // find relative minima (argrelmin). If none, take first index of region
            for pr in slices {
                if let Some((start, end)) = pr {
                    let len = end - start + 1;
                    if len > min_size {
                        let seg = (0..len)
                            .map(|j|yd2[start + j])
                            .collect::<Vec<Real>>();
                        let relm = argrelmin_segment(&seg);
                        if relm.is_empty() {
                            ipks.push(start); // fallback to first element in region
                        } else {
                            for r in relm {
                                ipks.push(start + r);
                            }
                        }
                    }
                }
            }
        },
        None => {
            // For each labeled region, take the index of the global minimum (argmin of yd2) within the region
            ipks = slices.iter().flatten()
                .map(|(start, end)| {
                        let mut best_i = *start;
                        let mut best_v = yd2[*start];
                        for (i, &yd2) in yd2.iter().enumerate().take(*end + 1).skip(*start) {
                            if yd2 < best_v {
                                best_v = yd2;
                                best_i = i;
                            }
                        }
                        best_i
                })
                .collect::<Vec<_>>();
            /*for pr in slices {
                if let Some((start, end)) = pr {
                    let mut best_i = start;
                    let mut best_v = yd2[start];
                    for i in (start + 1)..(end + 1) {
                        if yd2[i as usize] < best_v {
                            best_v = yd2[i];
                            best_i = i;
                        }
                    }
                    ipks.push(best_i as usize);
                }
            }*/
        },
    }

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
    let mut k = (0..size).map(|i|{
        let x = i as Real - radius as Real;
        Real::exp(-0.5 * x.powi(2) / s2)
    })
    .collect::<Vec<_>>();

    let sum = k.iter().sum::<Real>();
    k.iter_mut().for_each(|v| {
        *v /= sum;
    });
    return k;
}

// // Compute Gaussian kernel
// fn gaussian_kernel(sigma: Real) -> Vec<Real> {
//     if sigma <= 0.0 {
//         return vec![1.0];
//     }
//     let s2 = sigma * sigma;
//     let radius = i32::max(1, Real::ceil(3.0 * sigma) as i32);

//     let size = 2 * radius as usize + 1;
//     let mut k = (0..size).map(|i|{
//         let x = i as Real - radius as Real;
//         Real::exp(-0.5 * x.powi(2) / s2)
//     })
//     .collect::<Vec<_>>();

//     let sum = k.iter().sum::<Real>();
//     k.iter_mut().for_each(|v| {
//         *v /= sum;
//     });
//     return k;
// }

// function to reflect an index
fn reflect_index(idx: i32, n: usize) -> usize {
    if n == 0 {
        return 0;
    } else if idx < 0 {
        return (-idx - 1) as usize;
    } else if idx as usize >= n {
        return 2 * n - idx as usize - 1;
    }
    return idx as usize;
}


// // Gaussian Laplace filter
// fn convolve_reflect(data : &[Real], kernel : &[Real]) -> Vec<Real> {
//     let n = data.len();
//     let klen = kernel.len();
//     let radius = klen / 2;
//     let mut out = vec![0.0; n];
//     for i in 0..n {
//         let mut s = 0.0;
//         for k in 0..klen {
//             let j = i + (k - radius);
//             let jj = reflect_index(j as i32, n);
//             s += kernel[k] * data[jj];
//         }
//         out[i] = s;
//     }
//     return out;
// }


// Gaussian Laplace filter
fn convolve_reflect(data : &[Real], kernel : &[Real]) -> Vec<Real> {
    let data_length = data.len();
    let radius = kernel.len() as i32 / 2;
    (0..data_length).map(|i|
        kernel.iter().enumerate().map(|(k, coef)|
            coef*data[reflect_index((i + k) as i32 - radius, data_length)]
        ).sum()
    )
    .collect()
}

// Compute percentile
fn percentile(v: &[Real], p: Real) -> Result<Real, String> {
    if v.is_empty() {
        return Err("percentile: empty input".into());
    }
    let real_cmp = |a: &Real, b: &Real|a.partial_cmp(b).expect("Values are numbers, this should never fail");
    if p <= 0.0 {
        return Ok(v.iter().copied().min_by(real_cmp).expect("Min exists, this should never fail"));
    }
    if p >= 100.0 {
        return Ok(v.iter().copied().max_by(real_cmp).expect("Max exists, this should never fail"));
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
    let mean: Real = v.iter().sum::<Real>()/v.len() as Real;
    v.iter()
        .map(|&x: &Real| (x - mean).powi(2))
        .sum::<Real>()
        .powf(0.5)
    / v.len() as Real // FIXME: Should this not be divide by n - 1?
}

/// find indices of relative minima in a vector segment [0..n-1] (returns indices relative to segment start)
fn argrelmin_segment(seg: &[Real]) -> Vec<usize> {
    /*if (seg.len() <= 2) {
        return Default::default(); // no interior points
    }*/
    (1..(seg.len() - 1))
        .filter(|&i| seg[i] < seg[i - 1] && seg[i] < seg[i + 1] )
        .collect()
}