use crate::RosyType;
use crate::{RE, VE, DA, CD};

/// Get the return type of NORM for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::VE() => Some(RosyType::RE()),
        t if *t == RosyType::DA() => Some(RosyType::RE()),
        t if *t == RosyType::CD() => Some(RosyType::RE()),
        _ => None,
    }
}

/// Trait for computing the norm of Rosy data types.
pub trait RosyNORM {
    type Output;
    fn rosy_norm(&self) -> anyhow::Result<Self::Output>;
}

/// NORM for vectors - L1 norm (sum of absolute values)
impl RosyNORM for VE {
    type Output = RE;
    fn rosy_norm(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.abs()).sum())
    }
}

/// NORM for DA - max coefficient abs (max norm)
impl RosyNORM for DA {
    type Output = RE;
    fn rosy_norm(&self) -> anyhow::Result<Self::Output> {
        Ok(self.coeffs_iter().into_iter().map(|(_, c)| c.abs()).fold(0.0f64, f64::max))
    }
}

/// NORM for CD - max coefficient abs
impl RosyNORM for CD {
    type Output = RE;
    fn rosy_norm(&self) -> anyhow::Result<Self::Output> {
        use crate::taylor::DACoefficient;
        Ok(self.coeffs_iter().into_iter().map(|(_, c)| c.abs()).fold(0.0f64, f64::max))
    }
}

impl RosyNORM for crate::RosyValue {
    type Output = RE;
    fn rosy_norm(&self) -> anyhow::Result<Self::Output> {
        self.clone().expect_ve()?.rosy_norm()
    }
}
