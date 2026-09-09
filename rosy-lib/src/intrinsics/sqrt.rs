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

/// Square root via the homogeneous recurrence
/// `√f_n = (f_n - Σ_{k=1}^{n-1} √f_k √f_{n-k}) / (2 √f₀)`.
fn da_sqrt(da: &DA) -> anyhow::Result<DA> {
    crate::taylor::compose::compose_sqrt(da)
}

