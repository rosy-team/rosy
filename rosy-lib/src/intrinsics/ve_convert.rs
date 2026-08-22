use crate::RosyType;
use crate::{CM, RE, VE};

/// Get the return type of VE() for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::VE()),
        t if *t == RosyType::CM() => Some(RosyType::VE()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        _ => None,
    }
}

/// Trait for converting Rosy data types to vectors (VE).
pub trait RosyVEConvert {
    fn rosy_ve_convert(&self) -> anyhow::Result<VE>;
}

/// RE -> VE (single-element vector)
impl RosyVEConvert for RE {
    fn rosy_ve_convert(&self) -> anyhow::Result<VE> {
        Ok(vec![*self])
    }
}

/// CM -> VE (two-vector of real, imaginary parts)
impl RosyVEConvert for CM {
    fn rosy_ve_convert(&self) -> anyhow::Result<VE> {
        Ok(vec![self.re, self.im])
    }
}

/// VE -> VE identity
impl RosyVEConvert for VE {
    fn rosy_ve_convert(&self) -> anyhow::Result<VE> {
        Ok(self.clone())
    }
}

impl RosyVEConvert for crate::RosyValue {
    fn rosy_ve_convert(&self) -> anyhow::Result<VE> {
        self.clone().expect_ve()
    }
}
