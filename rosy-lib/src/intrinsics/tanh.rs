use crate::RosyType;
use crate::{RE, VE, DA};

/// Get the return type of TANH for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing hyperbolic tangent of Rosy data types.
pub trait RosyTANH {
    type Output;
    fn rosy_tanh(&self) -> anyhow::Result<Self::Output>;
}

/// TANH for real numbers
impl RosyTANH for RE {
    type Output = RE;
    fn rosy_tanh(&self) -> anyhow::Result<Self::Output> {
        Ok(self.tanh())
    }
}

/// TANH for vectors (elementwise)
impl RosyTANH for VE {
    type Output = VE;
    fn rosy_tanh(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.tanh()).collect())
    }
}

/// TANH for DA (Taylor composition)
/// Uses: tanh(f) = sinh(f) / cosh(f)
impl RosyTANH for DA {
    type Output = DA;
    fn rosy_tanh(&self) -> anyhow::Result<Self::Output> {
        use crate::intrinsics::sinh::RosySINH;
        use crate::intrinsics::cosh::RosyCOSH;

        let sinh_f = self.rosy_sinh()?;
        let cosh_f = self.rosy_cosh()?;

        (&sinh_f / &cosh_f).map_err(|e| e)
    }
}

