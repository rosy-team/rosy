use crate::RosyType;
use crate::{RE, CM, VE, DA, CD};

/// Get the return type of COS for a given input type.
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

/// Trait for computing cosine of Rosy data types.
pub trait RosyCOS {
    type Output;
    fn rosy_cos(&self) -> anyhow::Result<Self::Output>;
}

/// COS for real numbers
impl RosyCOS for RE {
    type Output = RE;
    fn rosy_cos(&self) -> anyhow::Result<Self::Output> {
        Ok(self.cos())
    }
}

/// COS for complex numbers
impl RosyCOS for CM {
    type Output = CM;
    fn rosy_cos(&self) -> anyhow::Result<Self::Output> {
        Ok(self.cos())
    }
}

/// COS for vectors (elementwise)
impl RosyCOS for VE {
    type Output = VE;
    fn rosy_cos(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.cos()).collect())
    }
}

/// COS for DA (Taylor composition)
impl RosyCOS for DA {
    type Output = DA;
    fn rosy_cos(&self) -> anyhow::Result<Self::Output> {
        da_cos(self)
    }
}

/// COS for CD (complex Taylor composition)
impl RosyCOS for CD {
    type Output = CD;
    fn rosy_cos(&self) -> anyhow::Result<Self::Output> {
        cd_cos(self)
    }
}

/// Compute cosine of a DA object using Taylor series composition.
///
/// Evaluates P(δf) = c₀ + δf·(c₁ + δf·(c₂ + ...)) using Horner's method
/// where c_n = d^n(cos)(f₀)/n!, cycle: [cos, -sin, -cos, sin]
fn da_cos(da: &DA) -> anyhow::Result<DA> {
    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    let da_prime = da.make_prime();

    // DACE-style recurrence: xf[i] = -xf[i-2] / (i*(i-1))
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(f0.cos());
    if nocut >= 1 { xf.push(-f0.sin()); }
    for i in 2..=nocut {
        xf.push(-xf[i - 2] / ((i * (i - 1)) as f64));
    }

    DA::horner_eval_with_rt(&da_prime, &xf, &rt)
}

/// Compute cosine of a CD object using Horner's method.
fn cd_cos(cd: &CD) -> anyhow::Result<CD> {
    
    use num_complex::Complex64;

    let config = crate::taylor::get_config()?;
    let nocut = config.max_order as usize;

    let f0 = cd.constant_part();
    let cd_prime = cd.make_prime();

    // DACE-style recurrence for complex cos coefficients
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(f0.cos());
    if nocut >= 1 { xf.push(-f0.sin()); }
    for i in 2..=nocut {
        xf.push(-xf[i - 2] / Complex64::new((i * (i - 1)) as f64, 0.0));
    }

    CD::horner_eval(&cd_prime, &xf)
}

