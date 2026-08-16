use crate::RosyType;
use crate::{RE, CM, VE, DA};

/// Get the return type of SINH for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::CM()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing hyperbolic sine of Rosy data types.
pub trait RosySINH {
    type Output;
    fn rosy_sinh(&self) -> anyhow::Result<Self::Output>;
}

/// SINH for real numbers
impl RosySINH for RE {
    type Output = RE;
    fn rosy_sinh(&self) -> anyhow::Result<Self::Output> {
        Ok(self.sinh())
    }
}

/// SINH for complex numbers
impl RosySINH for CM {
    type Output = CM;
    fn rosy_sinh(&self) -> anyhow::Result<Self::Output> {
        Ok(self.sinh())
    }
}

/// SINH for vectors (elementwise)
impl RosySINH for VE {
    type Output = VE;
    fn rosy_sinh(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.sinh()).collect())
    }
}

/// SINH for DA (Taylor composition)
impl RosySINH for DA {
    type Output = DA;
    fn rosy_sinh(&self) -> anyhow::Result<Self::Output> {
        da_sinh(self)
    }
}

/// Compute hyperbolic sine of a DA object using Horner's method.
///
/// `c_n = [sinh_f0, cosh_f0][n%2] / n!`
fn da_sinh(da: &DA) -> anyhow::Result<DA> {
    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    let da_prime = da.make_prime();

    // Recurrence: xf[i] = xf[i-2] / (i*(i-1))  (no negation for sinh/cosh)
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(f0.sinh());
    if nocut >= 1 { xf.push(f0.cosh()); }
    for i in 2..=nocut {
        xf.push(xf[i - 2] / ((i * (i - 1)) as f64));
    }

    DA::horner_eval_with_rt(&da_prime, &xf, &rt)
}

