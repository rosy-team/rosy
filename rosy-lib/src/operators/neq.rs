//! Not-equals operator for Rosy types.

use anyhow::Result;
use crate::RosyType;
use crate::{RE, ST, LO};

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    crate::operators::eq::get_return_type(lhs, rhs)
}

pub trait RosyNeq<Rhs = Self> {
    type Output;
    fn rosy_neq(self, rhs: Rhs) -> Result<Self::Output>;
}

// RE # RE (exact IEEE-754, matches COSY behavior)
impl RosyNeq<&RE> for &RE {
    type Output = LO;
    fn rosy_neq(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self != rhs)
    }
}

// ST # ST (exact string inequality)
impl RosyNeq<&ST> for &ST {
    type Output = LO;
    fn rosy_neq(self, rhs: &ST) -> Result<Self::Output> {
        Ok(self != rhs)
    }
}

// LO # LO (logical inequality)
impl RosyNeq<&LO> for &LO {
    type Output = LO;
    fn rosy_neq(self, rhs: &LO) -> Result<Self::Output> {
        Ok(self != rhs)
    }
}
