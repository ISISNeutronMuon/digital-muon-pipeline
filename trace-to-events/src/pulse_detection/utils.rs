use crate::pulse_detection::Real;

#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn stddev<I>(v: I) -> Result<Real, &'static str>
where
    I: Iterator<Item = Real> + ExactSizeIterator + Clone
{
    let len = v.len();
    if len == 0 {
        Err("Cannot compute standard deviation")
    } else if len == 1 {
        Ok(0.0)
    } else {
        let mean = v.clone().sum::<Real>() / len as Real;
        let var = v.map(|x: I::Item| (x - mean).powi(2)).sum::<Real>()
            / (len - 1) as Real;
        Ok(var.sqrt())
    }
}
pub(crate) fn stddev_from_slice(v: &[Real]) -> Result<Real, &'static str>
{
    let len = v.len();
    if len == 0 {
        Err("Cannot compute standard deviation")
    } else if len == 1 {
        Ok(0.0)
    } else {
        let mean = v.iter().sum::<Real>() / len as Real;
        let var = v.into_iter().map(|x| (x - mean).powi(2)).sum::<Real>()
            / (len - 1) as Real;
        Ok(var.sqrt())
    }
}