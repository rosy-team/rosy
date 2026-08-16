use crate::RosyType;
use crate::{RE, CM, DA, CD};

/// Get the return type of REAL for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::RE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        t if *t == RosyType::CD() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing the real part of Rosy data types.
pub trait RosyREAL {
    type Output;
    fn rosy_real(&self) -> anyhow::Result<Self::Output>;
}

/// REAL for real numbers - identity
impl RosyREAL for RE {
    type Output = RE;
    fn rosy_real(&self) -> anyhow::Result<Self::Output> {
        Ok(*self)
    }
}

/// REAL for complex numbers - real part
impl RosyREAL for CM {
    type Output = RE;
    fn rosy_real(&self) -> anyhow::Result<Self::Output> {
        Ok(self.re)
    }
}

/// REAL for DA - identity
impl RosyREAL for DA {
    type Output = DA;
    fn rosy_real(&self) -> anyhow::Result<Self::Output> {
        Ok(self.clone())
    }
}

/// REAL for CD - extract real part of each complex coefficient
impl RosyREAL for CD {
    type Output = DA;
    fn rosy_real(&self) -> anyhow::Result<Self::Output> {
        Ok(self.real_part())
    }
}

