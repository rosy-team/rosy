use crate::RosyType;
use crate::{RE, CM, DA, CD};

/// Get the return type of IMAG for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::RE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        t if *t == RosyType::CD() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing the imaginary part of Rosy data types.
pub trait RosyIMAG {
    type Output;
    fn rosy_imag(&self) -> anyhow::Result<Self::Output>;
}

/// IMAG for real numbers - returns 0.0
impl RosyIMAG for RE {
    type Output = RE;
    fn rosy_imag(&self) -> anyhow::Result<Self::Output> {
        Ok(0.0)
    }
}

/// IMAG for complex numbers - imaginary part
impl RosyIMAG for CM {
    type Output = RE;
    fn rosy_imag(&self) -> anyhow::Result<Self::Output> {
        Ok(self.im)
    }
}

/// IMAG for DA - returns zero DA
impl RosyIMAG for DA {
    type Output = DA;
    fn rosy_imag(&self) -> anyhow::Result<Self::Output> {
        Ok(DA::from_coeff(0.0))
    }
}

/// IMAG for CD - extract imaginary part of each complex coefficient
impl RosyIMAG for CD {
    type Output = DA;
    fn rosy_imag(&self) -> anyhow::Result<Self::Output> {
        Ok(self.imag_part())
    }
}

