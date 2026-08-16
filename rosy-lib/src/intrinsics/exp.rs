use crate::RosyType;
use crate::{RE, CM, VE, DA, CD};

/// Get the return type of EXP for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::CM()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        t if *t == RosyType::CD() => Some(RosyType::CD()),
        _ => None,
    }
}

/// Trait for computing the exponential of Rosy data types.
pub trait RosyEXP {
    type Output;
    fn rosy_exp(&self) -> anyhow::Result<Self::Output>;
}

/// EXP for real numbers
impl RosyEXP for RE {
    type Output = RE;
    fn rosy_exp(&self) -> anyhow::Result<Self::Output> {
        Ok(self.exp())
    }
}

/// EXP for complex numbers
impl RosyEXP for CM {
    type Output = CM;
    fn rosy_exp(&self) -> anyhow::Result<Self::Output> {
        Ok(self.exp())
    }
}

/// EXP for vectors (elementwise)
impl RosyEXP for VE {
    type Output = VE;
    fn rosy_exp(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.exp()).collect())
    }
}

/// EXP for DA (Taylor composition)
impl RosyEXP for DA {
    type Output = DA;
    fn rosy_exp(&self) -> anyhow::Result<Self::Output> {
        da_exp(self)
    }
}

/// EXP for CD (complex Taylor composition)
impl RosyEXP for CD {
    type Output = CD;
    fn rosy_exp(&self) -> anyhow::Result<Self::Output> {
        cd_exp(self)
    }
}

/// Compute exponential of a DA object using Horner's method.
///
/// exp(f) = exp(f₀) · (1 + δf + δf²/2! + ...) evaluated via Horner's.
fn da_exp(da: &DA) -> anyhow::Result<DA> {
    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    let exp_f0 = f0.exp();
    let da_prime = da.make_prime();

    // DACE-style recurrence: xf[i] = xf[i-1] / i
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(1.0);
    for i in 1..=nocut {
        xf.push(xf[i - 1] / (i as f64));
    }

    let mut result = DA::horner_eval_with_rt(&da_prime, &xf, &rt)?;
    result = (&result * DA::from_coeff(exp_f0))?;
    Ok(result)
}

/// Compute exponential of a CD object using Horner's method.
fn cd_exp(cd: &CD) -> anyhow::Result<CD> {
    
    use num_complex::Complex64;

    let config = crate::taylor::get_config()?;
    let nocut = config.max_order as usize;

    let f0 = cd.constant_part();
    let exp_f0 = f0.exp();
    let cd_prime = cd.make_prime();

    // DACE-style recurrence: xf[i] = xf[i-1] / i
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(Complex64::new(1.0, 0.0));
    for i in 1..=nocut {
        xf.push(xf[i - 1] / Complex64::new(i as f64, 0.0));
    }

    let mut result = CD::horner_eval(&cd_prime, &xf)?;
    result = (&result * CD::from_coeff(exp_f0))?;
    Ok(result)
}

