use crate::RosyType;
use crate::{RE, VE};

/// Get the return type of INT for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        _ => None,
    }
}

/// Trait for truncating Rosy data types toward zero.
pub trait RosyINT {
    type Output;
    fn rosy_int(&self) -> anyhow::Result<Self::Output>;
}

/// INT for real numbers - truncate toward zero
impl RosyINT for RE {
    type Output = RE;
    fn rosy_int(&self) -> anyhow::Result<RE> {
        Ok(self.trunc())
    }
}

/// INT for vectors - elementwise truncation
impl RosyINT for VE {
    type Output = VE;
    fn rosy_int(&self) -> anyhow::Result<VE> {
        Ok(self.iter().map(|x| x.trunc()).collect())
    }
}
