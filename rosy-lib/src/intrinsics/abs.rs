use crate::RosyType;
use crate::{RE, CM, VE, DA, CD};

/// Get the return type of ABS for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::RE()),
        t if *t == RosyType::DA() => Some(RosyType::RE()),
        t if *t == RosyType::CD() => Some(RosyType::RE()),
        _ => None,
    }
}

/// Trait for computing the absolute value of Rosy data types.
pub trait RosyABS {
    type Output;
    fn rosy_abs(&self) -> anyhow::Result<Self::Output>;
}

/// ABS for real numbers
impl RosyABS for RE {
    type Output = RE;
    fn rosy_abs(&self) -> anyhow::Result<RE> {
        Ok(self.abs())
    }
}

/// ABS for complex numbers - returns the modulus (norm)
impl RosyABS for CM {
    type Output = RE;
    fn rosy_abs(&self) -> anyhow::Result<RE> {
        Ok(self.norm())
    }
}

/// ABS for vectors - returns sum of absolute values
impl RosyABS for VE {
    type Output = RE;
    fn rosy_abs(&self) -> anyhow::Result<RE> {
        Ok(self.iter().map(|x| x.abs()).sum())
    }
}

/// ABS for DA - returns max absolute value among all coefficients
impl RosyABS for DA {
    type Output = RE;
    fn rosy_abs(&self) -> anyhow::Result<RE> {
        let max_coeff = self.coeffs_iter()
            .into_iter()
            .map(|(_, c)| c.abs())
            .fold(0.0_f64, f64::max);
        Ok(max_coeff)
    }
}

/// ABS for CD - returns max absolute value (norm) among all complex coefficients
impl RosyABS for CD {
    type Output = RE;
    fn rosy_abs(&self) -> anyhow::Result<RE> {
        use crate::taylor::DACoefficient;
        Ok(self.coeffs_iter().into_iter().map(|(_, c)| c.abs()).fold(0.0_f64, f64::max))
    }
}
