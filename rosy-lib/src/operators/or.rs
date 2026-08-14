//! Logical OR operator for Rosy types.
//!
//! This module provides the `RosyOr` trait and implementations.

use anyhow::Result;
use crate::{RosyType, RosyBaseType, LO};

/// Get the return type for OR operator.
pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    match (&lhs.base_type, &rhs.base_type) {
        (RosyBaseType::LO, RosyBaseType::LO) => Some(RosyType::new(RosyBaseType::LO, 0)),
        _ => None,
    }
}

pub trait RosyOr<Rhs = Self> {
    type Output;
    fn rosy_or(self, rhs: Rhs) -> Result<Self::Output>;
}

// LO OR LO
impl RosyOr<&LO> for &LO {
    type Output = LO;
    fn rosy_or(self, rhs: &LO) -> Result<Self::Output> {
        Ok(*self || *rhs)
    }
}
