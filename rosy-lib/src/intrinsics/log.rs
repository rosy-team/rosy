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
    crate::taylor::compose::compose_log(da)
}

