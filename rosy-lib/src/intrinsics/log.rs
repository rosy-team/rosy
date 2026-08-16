use crate::RosyType;
use crate::{RE, CM, VE, DA};

/// Get the return type of LOG for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::CM()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing the natural logarithm of Rosy data types.
pub trait RosyLOG {
    type Output;
    fn rosy_log(&self) -> anyhow::Result<Self::Output>;
}

/// LOG for real numbers (uses f64::ln — the natural log)
impl RosyLOG for RE {
    type Output = RE;
    fn rosy_log(&self) -> anyhow::Result<Self::Output> {
        Ok(self.ln())
    }
}

/// LOG for complex numbers
impl RosyLOG for CM {
    type Output = CM;
    fn rosy_log(&self) -> anyhow::Result<Self::Output> {
        Ok(self.ln())
    }
}

/// LOG for vectors (elementwise natural log)
impl RosyLOG for VE {
    type Output = VE;
    fn rosy_log(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.ln()).collect())
    }
}

/// LOG for DA (Taylor composition).
///
/// Uses: ln(f) = ln(f₀) + sum_{n=1}^{N} (-1)^(n+1) / n * (δf / f₀)^n
/// where f₀ is the constant part and δf = f - f₀.
impl RosyLOG for DA {
    type Output = DA;
    fn rosy_log(&self) -> anyhow::Result<Self::Output> {
        da_log(self)
    }
}

fn da_log(da: &DA) -> anyhow::Result<DA> {
    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    anyhow::ensure!(f0 != 0.0, "LOG: constant part of DA argument must be non-zero");

    let ln_f0 = f0.ln();
    let da_prime = da.make_prime();

    // u = δf / f₀
    let u = (&da_prime * DA::from_coeff(1.0 / f0))?;

    // ln(f) = ln(f₀) + u - u²/2 + u³/3 - ...
    // DACE-style: xf[0] = ln(f0), xf[n] = (-1)^(n+1) / n
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(ln_f0);
    for n in 1..=nocut {
        let sign = if n % 2 == 1 { 1.0 } else { -1.0 };
        xf.push(sign / (n as f64));
    }

    DA::horner_eval_with_rt(&u, &xf, &rt)
}

