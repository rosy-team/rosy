//! Logical NOT (negation) operator for Rosy types.
//!
//! This is a Rosy extension not present in COSY INFINITY.
//! Supports both `!x` and `NOT x` syntax.
//!
//! This module provides the `RosyNot` trait and implementations.

use crate::rosy_lib::{LO, RosyBaseType, RosyType};
use anyhow::Result;

/// Get the return type for NOT operator (unary).
pub fn get_return_type(operand: &RosyType) -> Option<RosyType> {
    match operand.base_type {
        RosyBaseType::LO => Some(RosyType::new(RosyBaseType::LO, 0)),
        _ => None,
    }
}

pub trait RosyNot {
    type Output;
    fn rosy_not(self) -> Result<Self::Output>;
}

// !LO (Logical NOT)
impl RosyNot for &LO {
    type Output = LO;
    fn rosy_not(self) -> Result<Self::Output> {
        Ok(!self)
    }
}
