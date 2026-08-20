use crate::{DA, RE};
use crate::RosyType;

/// Get the return type of ERF for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing the real error function of Rosy data types.
pub trait RosyERF {
    type Output;
    fn rosy_erf(&self) -> anyhow::Result<Self::Output>;
}

/// ERF for real numbers
impl RosyERF for RE {
    type Output = RE;
    fn rosy_erf(&self) -> anyhow::Result<Self::Output> {
        Ok(libm::erf(*self))
    }
}

impl RosyERF for crate::RosyValue {
    type Output = RE;
    fn rosy_erf(&self) -> anyhow::Result<Self::Output> {
        self.as_f64().rosy_erf()
    }
}

/// ERF for DA (Taylor composition).
///
/// ERF'(x) = (2/sqrt(pi)) * exp(-x^2)
///
/// The Taylor coefficients satisfy a recurrence derived from:
///   d/dx erf(f) = (2/sqrt(pi)) * exp(-f^2)
///
/// Using the DACE-style approach, we compute coefficients of erf expanded
/// around f0 using the recurrence for exp(-x^2) scaled by 2/sqrt(pi):
///
///   Let g(x) = exp(-x^2). Then g'(x) = -2x * g(x).
///   Taylor coefficients of g satisfy: (n+1)*g[n+1] = -2 * sum_{k=0}^{n} (k+1)*x[k+1]*g[n-k]
///   where x\[k\] are coefficients of the identity (x\[1\]=1, rest 0).
///
/// For composition erf(f0 + delta), we use:
///   erf_coeffs\[0\] = erf(f0)
///   erf_coeffs\[n\] = g_coeffs\[n-1\] / n   for n >= 1
/// where g_coeffs are coefficients of (2/sqrt(pi))*exp(-(f0+t)^2) expanded in t.
impl RosyERF for DA {
    type Output = DA;
    fn rosy_erf(&self) -> anyhow::Result<Self::Output> {
        da_erf(self)
    }
}

/// Compute erf of a DA object using Horner's method.
///
/// erf(f) = erf(f0) + integral_0^{delta f} (2/sqrt(pi))*exp(-(f0+t)^2) dt
///
/// The coefficients of the integrand expanded in delta are those of
/// (2/sqrt(pi))*exp(-f0^2)*exp(-2*f0*delta - delta^2).
///
/// We compute these as: c\[n\] = (2/sqrt(pi)) * exp(-f0^2) * p\[n\]
/// where p\[n\] are the Taylor coefficients of exp(-2*f0*t - t^2).
/// Then erf_coeffs\[n\] = c\[n-1\] / n for n >= 1, erf_coeffs\[0\] = erf(f0).
fn da_erf(da: &DA) -> anyhow::Result<DA> {
    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    let da_prime = da.make_prime();

    // Coefficients of exp(-2*f0*t - t^2) via recurrence.
    // Let h(t) = exp(-2*f0*t - t^2). Then h'(t) = (-2*f0 - 2*t)*h(t).
    // (n+1)*h[n+1] = -2*f0*h[n] - 2*h[n-1]    (for n >= 1)
    // (1)*h[1] = -2*f0*h[0]
    let mut h = vec![0.0f64; nocut + 2];
    h[0] = 1.0;
    if nocut >= 1 {
        h[1] = -2.0 * f0;
    }
    for n in 1..nocut {
        h[n + 1] = (-2.0 * f0 * h[n] - 2.0 * h[n - 1]) / ((n + 1) as f64);
    }

    // Scale by (2/sqrt(pi)) * exp(-f0^2)
    let scale = (2.0 / std::f64::consts::PI.sqrt()) * (-f0 * f0).exp();
    let c: Vec<f64> = h.iter().map(|&v| v * scale).collect();

    // Integrate: erf_coeffs[0] = erf(f0), erf_coeffs[n] = c[n-1] / n
    let mut xf = vec![0.0f64; nocut + 1];
    xf[0] = libm::erf(f0);
    for n in 1..=nocut {
        xf[n] = c[n - 1] / (n as f64);
    }

    DA::horner_eval_with_rt(&da_prime, &xf, &rt)
}
