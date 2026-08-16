use crate::RosyType;
use crate::{RE, CM, VE, DA};

/// Get the return type of COSH for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::CM()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing hyperbolic cosine of Rosy data types.
pub trait RosyCOSH {
    type Output;
    fn rosy_cosh(&self) -> anyhow::Result<Self::Output>;
}

/// COSH for real numbers
impl RosyCOSH for RE {
    type Output = RE;
    fn rosy_cosh(&self) -> anyhow::Result<Self::Output> {
        Ok(self.cosh())
    }
}

/// COSH for complex numbers
impl RosyCOSH for CM {
    type Output = CM;
    fn rosy_cosh(&self) -> anyhow::Result<Self::Output> {
        Ok(self.cosh())
    }
}

/// COSH for vectors (elementwise)
impl RosyCOSH for VE {
    type Output = VE;
    fn rosy_cosh(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.cosh()).collect())
    }
}

/// COSH for DA (Taylor composition)
impl RosyCOSH for DA {
    type Output = DA;
    fn rosy_cosh(&self) -> anyhow::Result<Self::Output> {
        da_cosh(self)
    }
}

/// Compute hyperbolic cosine of a DA object using Horner's method.
///
/// `c_n = [cosh_f0, sinh_f0][n%2] / n!`
fn da_cosh(da: &DA) -> anyhow::Result<DA> {
    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    let da_prime = da.make_prime();

    // Recurrence: xf[i] = xf[i-2] / (i*(i-1))  (no negation for cosh/sinh)
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(f0.cosh());
    if nocut >= 1 { xf.push(f0.sinh()); }
    for i in 2..=nocut {
        xf.push(xf[i - 2] / ((i * (i - 1)) as f64));
    }

    DA::horner_eval_with_rt(&da_prime, &xf, &rt)
}

