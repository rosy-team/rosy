use crate::RosyType;
use crate::{RE, VE, DA};

/// Get the return type of ACOS for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing arccosine of Rosy data types.
pub trait RosyACOS {
    type Output;
    fn rosy_acos(&self) -> anyhow::Result<Self::Output>;
}

/// ACOS for real numbers
impl RosyACOS for RE {
    type Output = RE;
    fn rosy_acos(&self) -> anyhow::Result<Self::Output> {
        Ok(self.acos())
    }
}

/// ACOS for vectors (elementwise)
impl RosyACOS for VE {
    type Output = VE;
    fn rosy_acos(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.acos()).collect())
    }
}

/// ACOS for DA (Taylor composition)
///
/// acos(x) = pi/2 - asin(x), so derivatives are the negatives of asin derivatives
/// (except the constant term).
impl RosyACOS for DA {
    type Output = DA;
    fn rosy_acos(&self) -> anyhow::Result<Self::Output> {
        da_acos(self)
    }
}

/// Compute arccosine of a DA object using Taylor series composition.
///
/// Uses the identity acos(x) = pi/2 - asin(x), so derivatives of acos are
/// the negative of the derivatives of asin (for n >= 1).
fn da_acos(da: &DA) -> anyhow::Result<DA> {
    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    let da_prime = da.make_prime();

    // Compute derivatives of acos at f0 (same recurrence as asin, but derivs[1] = -1/sqrt(1-x^2))
    let mut derivs = vec![0.0f64; nocut + 1];
    derivs[0] = f0.acos();
    if nocut >= 1 {
        let denom = (1.0 - f0 * f0).sqrt();
        if denom.abs() < 1e-15 {
            return Err(anyhow::anyhow!("ACOS is not differentiable at x = ±1"));
        }
        derivs[1] = -1.0 / denom;
    }
    if nocut >= 2 {
        let denom = 1.0 - f0 * f0;
        if denom.abs() < 1e-15 {
            return Err(anyhow::anyhow!("ACOS is not differentiable at x = ±1"));
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

