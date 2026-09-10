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

/// Hyperbolic sine via the coupled sinh/cosh homogeneous recurrence.
fn da_sinh(da: &DA) -> anyhow::Result<DA> {
    crate::taylor::compose::compose_sinh(da)
}

