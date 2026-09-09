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

/// Hyperbolic cosine via the coupled sinh/cosh homogeneous recurrence.
fn da_cosh(da: &DA) -> anyhow::Result<DA> {
    crate::taylor::compose::compose_cosh(da)
}

