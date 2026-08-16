use crate::RosyType;
use crate::{RE, VE, DA};

/// Get the return type of ASIN for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing arcsine of Rosy data types.
pub trait RosyASIN {
    type Output;
    fn rosy_asin(&self) -> anyhow::Result<Self::Output>;
}

/// ASIN for real numbers
impl RosyASIN for RE {
    type Output = RE;
    fn rosy_asin(&self) -> anyhow::Result<Self::Output> {
        Ok(self.asin())
    }
}

/// ASIN for vectors (elementwise)
impl RosyASIN for VE {
    type Output = VE;
    fn rosy_asin(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.asin()).collect())
    }
}

/// ASIN for DA (Taylor composition)
///
/// Uses Taylor series: asin(f₀ + δf) = asin(f₀) + Σ (d^n/dx^n asin(x)|_{x=f₀} / n!) * (δf)^n
/// asin'(x) = 1/sqrt(1-x²), higher derivatives computed numerically via recurrence.
impl RosyASIN for DA {
    type Output = DA;
    fn rosy_asin(&self) -> anyhow::Result<Self::Output> {
        da_asin(self)
    }
}

/// Compute arcsine of a DA object using Taylor series composition.
fn da_asin(da: &DA) -> anyhow::Result<DA> {
    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    let da_prime = da.make_prime();

    // Compute derivatives of asin at f0 via recurrence
    let mut derivs = vec![0.0f64; nocut + 1];
    derivs[0] = f0.asin();
    if nocut >= 1 {
        let denom = (1.0 - f0 * f0).sqrt();
        if denom.abs() < 1e-15 {
            return Err(anyhow::anyhow!("ASIN is not differentiable at x = ±1"));
        }
        derivs[1] = 1.0 / denom;
    }
    if nocut >= 2 {
        let denom = 1.0 - f0 * f0;
        if denom.abs() < 1e-15 {
            return Err(anyhow::anyhow!("ASIN is not differentiable at x = ±1"));
        }
        for n in 1..nocut {
            let n_f = n as f64;
            derivs[n + 1] = ((2.0 * n_f - 1.0) * f0 * derivs[n] + (n_f - 1.0).powi(2) * derivs[n - 1]) / (1.0 - f0 * f0);
        }
    }

    // Taylor coefficients c_n = derivs[n] / n!
    let mut xf = Vec::with_capacity(nocut + 1);
    let mut factorial = 1.0;
    for n in 0..=nocut {
        if n > 0 { factorial *= n as f64; }
        xf.push(derivs[n] / factorial);
    }

    DA::horner_eval_with_rt(&da_prime, &xf, &rt)
}

