//! Logical AND operator for Rosy types.
//!
//! This module provides the `RosyAnd` trait and implementations.

use crate::rosy_lib::{LO, RosyBaseType, RosyType};
use anyhow::Result;

/// Get the return type for AND operator.
pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    match (&lhs.base_type, &rhs.base_type) {
        (RosyBaseType::LO, RosyBaseType::LO) => Some(RosyType::new(RosyBaseType::LO, 0)),
        _ => None,
    }
}

pub trait RosyAnd<Rhs = Self> {
    type Output;
    fn rosy_and(self, rhs: Rhs) -> Result<Self::Output>;
}

// LO AND LO
impl RosyAnd<&LO> for &LO {
    type Output = LO;
    fn rosy_and(self, rhs: &LO) -> Result<Self::Output> {
        Ok(*self && *rhs)
    }
}
