//! Equality operator for Rosy types.

use anyhow::Result;
use crate::{RosyType, RosyBaseType};
use crate::intrinsics::RosyST;
use crate::{RE, ST, LO};

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    match crate::operators::dim0(lhs, rhs)? {
        (RosyBaseType::RE, RosyBaseType::RE)
        | (RosyBaseType::ST, RosyBaseType::ST)
        | (RosyBaseType::LO, RosyBaseType::LO)
        | (RosyBaseType::RE, RosyBaseType::ST)
        | (RosyBaseType::ST, RosyBaseType::RE) => Some(RosyType::LO()),
        _ => None,
    }
}

pub trait RosyEq<Rhs = Self> {
    type Output;
    fn rosy_eq(self, rhs: Rhs) -> Result<Self::Output>;
}

// RE = RE (exact IEEE-754, matches COSY behavior)
impl RosyEq<&RE> for &RE {
    type Output = LO;
    fn rosy_eq(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self == rhs)
    }
}

// ST = ST (exact string equality)
impl RosyEq<&ST> for &ST {
    type Output = LO;
    fn rosy_eq(self, rhs: &ST) -> Result<Self::Output> {
        Ok(self == rhs)
    }
}

impl RosyEq<&ST> for &RE {
    type Output = LO;
    fn rosy_eq(self, rhs: &ST) -> Result<Self::Output> {
        Ok(&self.rosy_to_string() == rhs)
    }
}

impl RosyEq<&RE> for &ST {
    type Output = LO;
    fn rosy_eq(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self == &rhs.rosy_to_string())
    }
}

// LO = LO (logical equality)
impl RosyEq<&LO> for &LO {
    type Output = LO;
    fn rosy_eq(self, rhs: &LO) -> Result<Self::Output> {
        Ok(self == rhs)
    }
}
