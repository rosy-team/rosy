use crate::RosyType;
use crate::{RE, CM, VE, DA};

/// Get the return type of SQRT for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::CM()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing the square root of Rosy data types.
pub trait RosySQRT {
    type Output;
    fn rosy_sqrt(&self) -> anyhow::Result<Self::Output>;
}

/// SQRT for real numbers
impl RosySQRT for RE {
    type Output = RE;
    fn rosy_sqrt(&self) -> anyhow::Result<Self::Output> {
        Ok(self.sqrt())
    }
}

/// SQRT for complex numbers
impl RosySQRT for CM {
    type Output = CM;
    fn rosy_sqrt(&self) -> anyhow::Result<Self::Output> {
        Ok(self.sqrt())
    }
}

/// SQRT for vectors (elementwise)
impl RosySQRT for VE {
    type Output = VE;
    fn rosy_sqrt(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.sqrt()).collect())
    }
}

/// SQRT for DA (Taylor composition via binomial series)
impl RosySQRT for DA {
    type Output = DA;
    fn rosy_sqrt(&self) -> anyhow::Result<Self::Output> {
        da_sqrt(self)
    }
}

/// Compute square root of a DA object using binomial series expansion.
///
/// Uses: sqrt(f) = sqrt(f0) * sqrt(1 + u)  where u = (f - f0) / f0
/// sqrt(1 + u) = sum_{n=0}^{N} C(1/2, n) * u^n
/// where C(1/2, n) = (1/2)(1/2-1)...(1/2-n+1) / n!
///
/// Requires: f0 = constant part of the DA > 0 (sqrt is not analytic at 0).
fn da_sqrt(da: &DA) -> anyhow::Result<DA> {
    

    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    anyhow::ensure!(f0 > 0.0, "SQRT: constant part of DA must be positive, got {}", f0);

    let sqrt_f0 = f0.sqrt();
    let da_prime = da.make_prime();
    let da_delta = (&da_prime * DA::from_coeff(1.0 / f0))?;

    // Binomial coefficients C(1/2, n) via recurrence
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(1.0);
    let mut binom_coeff = 0.5_f64;
    for n in 1..=nocut {
        xf.push(binom_coeff);
        binom_coeff *= (0.5 - n as f64) / (n as f64 + 1.0);
    }

    let mut result = DA::horner_eval_with_rt(&da_delta, &xf, &rt)?;
    result = (&result * DA::from_coeff(sqrt_f0))?;
    Ok(result)
}

