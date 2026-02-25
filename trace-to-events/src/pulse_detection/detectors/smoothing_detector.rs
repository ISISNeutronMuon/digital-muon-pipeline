//! This detector registers an event whenever the derivative of the input stream [FIXME]
//! This algorithm uses: Richard Waite's second derivative smoothing peak finder,
//! translated from C++ by Boris Shustin and Jaroslav Fowlkes for ALC, and translated
//! thereafter into Rust.

use std::ops::Range;

use crate::pulse_detection::Real;
use tracing::instrument;

/// Second derivative smoothing peak finder
/// # Parameters
/// - x: x data.
/// - y: y data.
/// - noise_centile: centile of x to use for noise estimation.
/// - kernel_sigma: sigma of the Gaussian kernel for smoothing.
/// - nsig_noise: number of standard deviations above noise to use as threshold.
/// - min_size: minimum size of region to consider a peak, if absent all regions are considered.
///
/// # Return
/// (x_peaks, y_peaks) - 1D arrays of peak locations
///
#[instrument(skip_all)]
pub(crate) fn sec_deriv_smoothing_for_peaks(
    x: &[Real],
    y: &[Real],
    noise_centile: Real,
    kernel_sigma: Real,
    nsig_noise: Real,
    min_size: Option<usize>,
) -> Result<(Vec<Real>, Vec<Real>), &'static str> {
    if x.len() != y.len() {
        return Err("x and y must have same length");
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
    let yd2: Vec<f64> = second_deriv(&y_smooth, kernel_sigma.powi(2));

    // 3. estimate noise from last portion of x (x > percentile(x, noise_centile))
    let percentile = ((yd2.len() as f64 * noise_centile) / 100.0) as usize;
    let noise_std = stddev(&yd2[percentile..yd2.len()])?;

    // 4. label contiguous regions where yd2 < -nsig_noise * noise_std
    let threshold = -nsig_noise * noise_std;
    let regions = find_region_bounds(&yd2, threshold);

    // 5. pick peaks
    // indices of peaks
    let ipks = match min_size {
        Some(min_size) => filter_minsize_and_find_minima(min_size, &yd2, &regions),
        None => find_minima_no_minsize(&yd2, &regions),
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
    Ok((xpk, ypk))
}

/// Computes the standard Gaussian kernel.
/// # Parameters
/// - sigma - standard deviation of the Gaussian curve.
/// # Returns
/// A vector of length 8*sigma + 1 (or 3, of sigma < 1.4),
/// with the values of the Gaussian centred on middle element.
/// This length is deemed sufficient to allow the curve to serve as a convolution window.
fn gaussian_kernel(sigma: Real) -> Vec<Real> {
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

/// Reflects the index if it is outside the given boundary `0,n`.
/// # Parameters
/// - idx: the index to reflect.
/// - n: the position of the right-sided boundary.
fn reflect_index(idx: i32, n: usize) -> usize {
    if n == 0 {
        0
    } else if idx < 0 {
        (-idx - 1) as usize
    } else if idx >= n as i32 {
        2 * n - idx as usize - 1
    } else {
        idx as usize
    }
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
                .map(|(kernel_idx, &coef)| {
                    coef * data[reflect_index(idx + (kernel_idx as i32 - radius), data_length)]
                })
                .sum()
        })
        .collect()
}

// 2.
fn second_deriv(data: &[Real], kernel_sigma_sqr: Real) -> Vec<Real> {
    (0..data.len())
        .map(|i| {
            let im = reflect_index(i as i32 - 1, data.len());
            let ip = reflect_index(i as i32 + 1, data.len());
            (data[im] - 2.0 * data[i] + data[ip]) * kernel_sigma_sqr
        })
        .collect::<Vec<_>>()
}

// 3.
// Compute standard deviation
fn stddev(v: &[Real]) -> Result<Real, &'static str> {
    if v.is_empty() {
        Err("Cannot compute standard deviation")
    } else if v.len() == 1 {
        Ok(0.0)
    } else {
        let mean: Real = v.iter().sum::<Real>() / v.len() as Real;
        let var =
            v.iter().map(|&x: &Real| (x - mean).powi(2)).sum::<Real>() / (v.len() as Real - 1.0);
        Ok(var.sqrt())
    }
}

// 4.
fn find_region_bounds(yd2: &[Real], threshold: Real) -> Vec<Range<usize>> {
    let closure = |(mut acc, next): (Vec<Range<usize>>, Option<_>),
                   (i, &yd2)|
     -> (Vec<Range<usize>>, Option<(usize, usize)>) {
        let new_next = {
            if yd2 < threshold {
                Some(next.map(|(start, _)| (start, i)).unwrap_or((i, i)))
            } else {
                if let Some(next) = next {
                    acc.push(next.0..next.1);
                }
                None
            }
        };
        (acc, new_next)
    };
    let (mut acc, next) = yd2.iter().enumerate().fold((Vec::new(), None), closure);
    if let Some(next) = next {
        acc.push(next.0..next.1);
    }
    acc
}

// 5.
fn find_minima_no_minsize(yd2: &[Real], regions: &[Range<usize>]) -> Vec<usize> {
    // For each labeled region, take the index of the global minimum (argmin of yd2) within the region
    regions
        .iter()
        .flat_map(|range| find_global_argmin(&yd2[range.clone()], range.start))
        .collect::<Vec<_>>()
}

fn find_global_argmin(yd2: &[Real], origin: usize) -> Option<usize> {
    if yd2.is_empty() {
        return None;
    }
    let argmin = yd2
        .iter()
        .enumerate()
        .fold((0, yd2[0]), |(best_i, best_v), (new_i, &new_v)| {
            if new_v < best_v {
                (new_i, new_v)
            } else {
                (best_i, best_v)
            }
        })
        .0
        + origin;
    Some(argmin)
}

fn filter_minsize_and_find_minima(
    min_size: usize,
    yd2: &[Real],
    regions: &[Range<usize>],
) -> Vec<usize> {
    regions
        .iter()
        .filter(|range| range.len() + 1 > min_size)
        .flat_map(|range| find_argminima(&yd2[range.clone()], range.start))
        .collect()
}

fn find_argminima(yd2: &[Real], start: usize) -> Vec<usize> {
    let relmin = argrelmin_segment(yd2);
    if relmin.is_empty() {
        vec![start] // fallback to first element in region
    } else {
        relmin.iter().map(|&r| start + r).collect()
    }
}

/// find indices of relative minima in a vector segment [0..n-1] (returns indices relative to segment start)
fn argrelmin_segment(seg: &[Real]) -> Vec<usize> {
    (1..(seg.len() - 1))
        .filter(|&i| seg[i] < seg[i - 1] && seg[i] < seg[i + 1])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_approx_eq::assert_approx_eq;

    // number of data points
    const NX: usize = 85;

    // data x values
    const X: [f64; NX] = [
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
    fn test_gaussian_kernel() {
        let kernel_sigma = 2.0;
        let kernel = gaussian_kernel(kernel_sigma);

        // Kernel should sum to unity.
        assert_eq!(kernel.iter().sum::<Real>(), 1.0);

        assert_eq!(kernel.len(), 17);

        const KERNEL_LEFT_SIDE: [f64; 9] = [
            6.691628957263553e-5,
            0.0004363490205067883,
            0.002215963172596555,
            0.00876430436278587,
            0.026995957967298846,
            0.06475993660472744,
            0.12098748976534904,
            0.17603575888479037,
            0.199474647864745,
        ];

        // Kernel should be symmetric about central value.
        for i in 0..9 {
            assert_eq!(kernel[i], KERNEL_LEFT_SIDE[i]);
            assert_eq!(kernel[i], kernel[16 - i]);
        }
    }

    #[test]
    fn test_kernel_convolution() {
        let kernel_sigma = 2.0;
        let kernel = gaussian_kernel(kernel_sigma);
        let y_smooth = convolve_reflect(&Y, &kernel);

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
        assert_eq!(SMOOTH.len(), y_smooth.len());

        for (y1, y2) in y_smooth.iter().zip(SMOOTH.iter()) {
            assert_eq!(y1, y2);
        }
    }

    #[test]
    fn test_second_deriv() {
        let kernel_sigma = 2.0;
        let kernel = gaussian_kernel(kernel_sigma);
        let y_smooth = convolve_reflect(&Y, &kernel);
        let yd2: Vec<f64> = second_deriv(&y_smooth, kernel_sigma);

        const SECOND_DERIV: [f64; NX] = [
            0.003933263371568241,
            0.0058963309645015255,
            0.008434437946255702,
            0.009160987309617813,
            0.006125514543818905,
            -0.0003435070816827368,
            -0.007412375800508325,
            -0.011808862912426582,
            -0.012202365316404729,
            -0.009620781329556793,
            -0.0060177381217643156,
            -0.0028179647974764244,
            -0.0005511723312205674,
            0.0007739274689881059,
            0.0013226388837605518,
            0.0013984273690708648,
            0.0013998296756597,
            0.0015183229635858664,
            0.0015413054212831678,
            0.001202537501647838,
            0.0006912001128772799,
            0.0005271929138423587,
            0.000888884035324472,
            0.0013153680093063586,
            0.0012044150642574192,
            0.0004850322349322783,
            -0.00028880420789619693,
            -0.0004016347155546898,
            0.0005119350617702606,
            0.002269625138561736,
            0.004296232575462848,
            0.005916412107941346,
            0.006513166924465741,
            0.005563651970045505,
            0.0028833093935210358,
            -0.0009208972358741019,
            -0.004492193792904414,
            -0.006656644874094841,
            -0.007301128779957977,
            -0.007056342708402524,
            -0.0062677635370256,
            -0.004721567209395761,
            -0.0023718002177020303,
            0.00010317272102536301,
            0.0017053545106165552,
            0.0019791383461893602,
            0.0013562888127616746,
            0.0006945229532265007,
            0.0005479791687722918,
            0.0008332718318881244,
            0.0011143655319005852,
            0.001144087218789884,
            0.001066507181117872,
            0.0010962137273217432,
            0.0011794916212150564,
            0.0011022399851171524,
            0.0008096279720504751,
            0.00044778212821224017,
            0.00016100485651266566,
            -2.6420134425546304e-5,
            -0.00013978512504587287,
            -0.00015595936736043092,
            -4.122224070759323e-5,
            0.00010824534855045226,
            8.52480093306851e-5,
            -0.00014519105954235306,
            -0.00029462161910108475,
            -8.362133674279931e-5,
            0.00033284957664631715,
            0.0004915658373166806,
            0.0001985708908035025,
            -0.00018820241951661432,
            -0.00018000754794006424,
            0.00023268410354045088,
            0.0005695477133972754,
            0.00043455876997824244,
            -8.470410691804181e-5,
            -0.0006147472926532616,
            -0.0008854732781200247,
            -0.0008468629082423254,
            -0.0005704711103244088,
            -0.0001627480531168242,
            0.00029735416249944413,
            0.0007495679196893139,
            0.0010583927090690136,
        ];
        assert_eq!(SECOND_DERIV.len(), yd2.len());

        for (y1, y2) in yd2.iter().zip(SECOND_DERIV.iter()) {
            assert_eq!(y1, y2);
        }
    }

    #[test]
    fn test_region() {
        let kernel_sigma = 2.0;
        let kernel = gaussian_kernel(kernel_sigma);
        let y_smooth = convolve_reflect(&Y, &kernel);
        let yd2: Vec<f64> = second_deriv(&y_smooth, kernel_sigma);

        // 3. estimate noise from last portion of x (x > percentile(x, noise_centile))
        let percentile = ((yd2.len() as f64 * 90.0) / 100.0) as usize;
        let noise_std = stddev(&yd2[percentile..yd2.len()]).unwrap();

        // 4. label contiguous regions where yd2 < -nsig_noise * noise_std
        let threshold = -5.0 * noise_std;
        let regions = find_region_bounds(&yd2, threshold);

        //find_region_bounds

        assert_eq!(regions, vec![6..10, 36..41]);
    }

    #[test]
    fn test_peaks_no_min() {
        let kernel_sigma = 2.0;
        let kernel = gaussian_kernel(kernel_sigma);
        let y_smooth = convolve_reflect(&Y, &kernel);
        let yd2: Vec<f64> = second_deriv(&y_smooth, kernel_sigma);

        // 3. estimate noise from last portion of x (x > percentile(x, noise_centile))
        let percentile = ((yd2.len() as f64 * 90.0) / 100.0) as usize;
        let noise_std = stddev(&yd2[percentile..yd2.len()]).unwrap();

        // 4. label contiguous regions where yd2 < -nsig_noise * noise_std
        let threshold = -5.0 * noise_std;
        let regions = find_region_bounds(&yd2, threshold);

        let minima = find_minima_no_minsize(&yd2, &regions);

        assert_eq!(minima, vec![8, 38]);
    }

    #[test]
    fn test_peaks_minsize() {
        let kernel_sigma = 2.0;
        let kernel = gaussian_kernel(kernel_sigma);
        let y_smooth = convolve_reflect(&Y, &kernel);
        let yd2: Vec<f64> = second_deriv(&y_smooth, kernel_sigma);

        // 3. estimate noise from last portion of x (x > percentile(x, noise_centile))
        let percentile = ((yd2.len() as f64 * 90.0) / 100.0) as usize;
        let noise_std = stddev(&yd2[percentile..yd2.len()]).unwrap();

        // 4. label contiguous regions where yd2 < -nsig_noise * noise_std
        let threshold = -5.0 * noise_std;
        let regions = find_region_bounds(&yd2, threshold);

        let minima = filter_minsize_and_find_minima(5, &yd2, &regions);

        assert_eq!(minima, vec![38]);
    }

    #[test]
    fn test_detector() {
        let (x, y) = sec_deriv_smoothing_for_peaks(&X, &Y, 90.0, 2.0, 5.0, Some(2)).unwrap();
        assert_eq!(x.len(), 2);
        assert_eq!(y.len(), 2);
        assert_approx_eq!(x[0], 3.6112);
        assert_approx_eq!(y[0], 0.1283464566929134);
        assert_approx_eq!(x[1], 3.6352);
        assert_approx_eq!(y[1], 0.09291338582677167);
    }
}
