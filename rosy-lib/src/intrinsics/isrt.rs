use crate::RosyType;
use crate::{RE, VE, DA};

/// Get the return type of ISRT for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing inverse square root of Rosy data types.
pub trait RosyISRT {
    type Output;
    fn rosy_isrt(&self) -> anyhow::Result<Self::Output>;
}

/// ISRT for real numbers: x^(-1/2) = 1/sqrt(x)
impl RosyISRT for RE {
    type Output = RE;
    fn rosy_isrt(&self) -> anyhow::Result<Self::Output> {
        Ok(1.0 / self.sqrt())
    }
}

/// ISRT for vectors (elementwise)
impl RosyISRT for VE {
    type Output = VE;
    fn rosy_isrt(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| 1.0 / x.sqrt()).collect())
    }
}

/// ISRT for DA (Taylor composition via binomial series with alpha = -0.5)
impl RosyISRT for DA {
    type Output = DA;
    fn rosy_isrt(&self) -> anyhow::Result<Self::Output> {
        da_isrt(self)
    }
}

/// Compute inverse square root of a DA object using binomial series.
///
/// Uses: f^alpha where alpha = -0.5
/// f^alpha = f0^alpha * (1 + u)^alpha  where u = (f - f0) / f0
/// (1 + u)^alpha = sum_{n=0}^{N} C(alpha, n) * u^n
/// C(alpha, n) = alpha*(alpha-1)*...*(alpha-n+1) / n!
fn da_isrt(da: &DA) -> anyhow::Result<DA> {
    

    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    anyhow::ensure!(f0 > 0.0, "ISRT: constant part of DA must be positive, got {}", f0);

    let alpha = -0.5_f64;
    let f0_alpha = f0.powf(alpha);
    let da_prime = da.make_prime();
    let da_delta = (&da_prime * DA::from_coeff(1.0 / f0))?;

    // Binomial coefficients C(alpha, n) via recurrence
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(1.0);
    let mut binom_coeff = alpha;
    for n in 1..=nocut {
        xf.push(binom_coeff);
        binom_coeff *= (alpha - n as f64) / (n as f64 + 1.0);
    }

    let mut result = DA::horner_eval_with_rt(&da_delta, &xf, &rt)?;
    result = (&result * DA::from_coeff(f0_alpha))?;
    Ok(result)
}

