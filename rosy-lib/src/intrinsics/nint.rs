use crate::RosyType;
use crate::{RE, VE};

/// Get the return type of NINT for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        _ => None,
    }
}

/// Trait for rounding Rosy data types to the nearest integer.
pub trait RosyNINT {
    type Output;
    fn rosy_nint(&self) -> anyhow::Result<Self::Output>;
}

/// NINT for real numbers - round to nearest integer
impl RosyNINT for RE {
    type Output = RE;
    fn rosy_nint(&self) -> anyhow::Result<RE> {
        Ok(self.round())
    }
}

/// NINT for vectors - elementwise rounding
impl RosyNINT for VE {
    type Output = VE;
    fn rosy_nint(&self) -> anyhow::Result<VE> {
        Ok(self.iter().map(|x| x.round()).collect())
    }
}
