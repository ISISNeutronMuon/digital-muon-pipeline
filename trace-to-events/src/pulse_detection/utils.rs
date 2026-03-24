use crate::pulse_detection::Real;

#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn std_dev(v: &[Real]) -> Result<Real, &'static str> {
    let len = v.len();
    if len == 0 {
        Err("Cannot compute standard deviation")
    } else if len == 1 {
        Ok(0.0)
    } else {
        let mean = v.iter().sum::<Real>() / len as Real;
        let var = v.iter().map(|x| (x - mean).powi(2)).sum::<Real>() / (len - 1) as Real;
        Ok(var.sqrt())
    }
}

#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn global_arg_min<A, B>(mut iter: impl Iterator<Item = (A, B)>) -> A
where
    B: PartialOrd,
{
    let first = iter
        .next()
        .expect("region should have nonzero size, this should never fail.");
    iter.fold(first, |min_so_far, (t1, v1)| {
        if v1 < min_so_far.1 {
            (t1, v1)
        } else {
            min_so_far
        }
    })
    .0
}
