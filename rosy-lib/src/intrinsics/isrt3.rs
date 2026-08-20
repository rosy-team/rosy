use crate::RosyType;
use crate::{RE, VE, DA};

/// Get the return type of ISRT3 for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing x^(-3/2) of Rosy data types.
pub trait RosyISRT3 {
    type Output;
    fn rosy_isrt3(&self) -> anyhow::Result<Self::Output>;
}

/// ISRT3 for real numbers: x^(-3/2)
impl RosyISRT3 for RE {
    type Output = RE;
    fn rosy_isrt3(&self) -> anyhow::Result<Self::Output> {
        Ok(self.powf(-1.5))
    }
}

/// ISRT3 for vectors (elementwise)
impl RosyISRT3 for VE {
    type Output = VE;
    fn rosy_isrt3(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.powf(-1.5)).collect())
    }
}

/// ISRT3 for DA (Taylor composition via binomial series with alpha = -1.5)
impl RosyISRT3 for DA {
    type Output = DA;
    fn rosy_isrt3(&self) -> anyhow::Result<Self::Output> {
        da_isrt3(self)
    }
}

impl RosyISRT3 for crate::RosyValue {
    type Output = VE;
    fn rosy_isrt3(&self) -> anyhow::Result<Self::Output> {
        self.clone().expect_ve()?.rosy_isrt3()
    }
}

/// Compute x^(-3/2) of a DA object using binomial series.
///
/// Uses: f^alpha where alpha = -1.5
/// f^alpha = f0^alpha * (1 + u)^alpha  where u = (f - f0) / f0
/// (1 + u)^alpha = sum_{n=0}^{N} C(alpha, n) * u^n
fn da_isrt3(da: &DA) -> anyhow::Result<DA> {
    

    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    anyhow::ensure!(f0 > 0.0, "ISRT3: constant part of DA must be positive, got {}", f0);

    let alpha = -1.5_f64;
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

