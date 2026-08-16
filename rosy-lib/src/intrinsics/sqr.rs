use crate::RosyType;
use crate::{RE, CM, VE, DA, CD};

/// Get the return type of SQR for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::CM()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        t if *t == RosyType::CD() => Some(RosyType::CD()),
        _ => None,
    }
}

/// Trait for computing the square of Rosy data types.
pub trait RosySQR {
    type Output;
    fn rosy_sqr(&self) -> anyhow::Result<Self::Output>;
}

/// SQR for real numbers
impl RosySQR for RE {
    type Output = RE;
    fn rosy_sqr(&self) -> anyhow::Result<Self::Output> {
        Ok(self * self)
    }
}

/// SQR for complex numbers
impl RosySQR for CM {
    type Output = CM;
    fn rosy_sqr(&self) -> anyhow::Result<Self::Output> {
        Ok(self * self)
    }
}

/// SQR for vectors (elementwise)
impl RosySQR for VE {
    type Output = VE;
    fn rosy_sqr(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x * x).collect())
    }
}

/// SQR for DA (Taylor multiplication)
impl RosySQR for DA {
    type Output = DA;
    fn rosy_sqr(&self) -> anyhow::Result<Self::Output> {
        (self * self).map_err(|e| e)
    }
}

/// SQR for CD (complex Taylor multiplication)
impl RosySQR for CD {
    type Output = CD;
    fn rosy_sqr(&self) -> anyhow::Result<Self::Output> {
        (self * self).map_err(|e| e)
    }
}
