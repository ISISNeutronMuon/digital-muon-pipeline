use crate::pulse_detection::Real;

#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn stddev_from_slice(v: &[Real]) -> Result<Real, &'static str>
{
    let len = v.len();
    if len == 0 {
        Err("Cannot compute standard deviation")
    } else if len == 1 {
        Ok(0.0)
    } else {
        let mean = v.iter().sum::<Real>() / len as Real;
        let var = v.iter().map(|x| (x - mean).powi(2)).sum::<Real>()
            / (len - 1) as Real;
        Ok(var.sqrt())
    }
}