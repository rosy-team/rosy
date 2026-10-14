//! Less-than-or-equal operator for Rosy types.
//!
//! This is a Rosy extension not present in COSY INFINITY.
//!
//! This module provides the `RosyLte` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `LTE_REGISTRY` constant below.

use crate::RosyType;
use crate::{LO, RE, ST};
use anyhow::Result;

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    crate::operators::lt::get_return_type(lhs, rhs)
}

pub trait RosyLte<Rhs = Self> {
    type Output;
    fn rosy_lte(self, rhs: Rhs) -> Result<Self::Output>;
}

// RE <= RE
impl RosyLte<&RE> for &RE {
    type Output = LO;
    fn rosy_lte(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self <= rhs)
    }
}

// ST <= ST (lexicographic ordering)
impl RosyLte<&ST> for &ST {
    type Output = LO;
    fn rosy_lte(self, rhs: &ST) -> Result<Self::Output> {
        Ok(self <= rhs)
    }
}
