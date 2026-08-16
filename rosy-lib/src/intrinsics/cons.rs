use crate::RosyType;
use crate::{RE, CM, VE, DA, CD};

/// Get the return type of CONS for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::CM()),
        t if *t == RosyType::VE() => Some(RosyType::RE()),
        t if *t == RosyType::DA() => Some(RosyType::RE()),
        t if *t == RosyType::CD() => Some(RosyType::CM()),
        _ => None,
    }
}

/// Trait for extracting the constant part of Rosy data types.
pub trait RosyCONS {
    type Output;
    fn rosy_cons(&self) -> anyhow::Result<Self::Output>;
}

/// CONS for real numbers - identity
impl RosyCONS for RE {
    type Output = RE;
    fn rosy_cons(&self) -> anyhow::Result<Self::Output> {
        Ok(*self)
    }
}

/// CONS for complex numbers - identity
impl RosyCONS for CM {
    type Output = CM;
    fn rosy_cons(&self) -> anyhow::Result<Self::Output> {
        Ok(*self)
    }
}

/// CONS for vectors - max abs value
impl RosyCONS for VE {
    type Output = RE;
    fn rosy_cons(&self) -> anyhow::Result<Self::Output> {
        if self.is_empty() {
            anyhow::bail!("CONS called on empty vector");
        }
        Ok(self.iter().map(|x| x.abs()).fold(0.0f64, f64::max))
    }
}

/// CONS for DA - constant part
impl RosyCONS for DA {
    type Output = RE;
    fn rosy_cons(&self) -> anyhow::Result<Self::Output> {
        Ok(self.constant_part())
    }
}

/// CONS for CD - constant part (complex)
impl RosyCONS for CD {
    type Output = CM;
    fn rosy_cons(&self) -> anyhow::Result<Self::Output> {
        Ok(self.constant_part())
    }
}
