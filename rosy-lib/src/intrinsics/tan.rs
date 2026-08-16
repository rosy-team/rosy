use crate::RosyType;
use crate::{RE, VE, DA};

/// Get the return type of TAN for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing the tangent of Rosy data types.
pub trait RosyTAN {
    type Output;
    fn rosy_tan(&self) -> anyhow::Result<Self::Output>;
}

/// TAN for real numbers
impl RosyTAN for RE {
    type Output = RE;
    fn rosy_tan(&self) -> anyhow::Result<Self::Output> {
        Ok(self.tan())
    }
}

/// TAN for vectors (elementwise)
impl RosyTAN for VE {
    type Output = VE;
    fn rosy_tan(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.tan()).collect())
    }
}

/// TAN for DA (Taylor composition)
/// Uses: tan(f) = sin(f) / cos(f)
impl RosyTAN for DA {
    type Output = DA;
    fn rosy_tan(&self) -> anyhow::Result<Self::Output> {
        use crate::intrinsics::sin::RosySIN;
        use crate::intrinsics::cos::RosyCOS;

        let sin_f = self.rosy_sin()?;
        let cos_f = self.rosy_cos()?;

        (&sin_f / &cos_f).map_err(|e| e)
    }
}

