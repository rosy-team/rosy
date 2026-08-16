//! Greater-than-or-equal operator for Rosy types.
//!
//! This is a Rosy extension not present in COSY INFINITY.
//!
//! This module provides the `RosyGte` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `GTE_REGISTRY` constant below.

use anyhow::Result;
use crate::RosyType;
use crate::{RE, ST, LO};

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    crate::operators::lt::get_return_type(lhs, rhs)
}

pub trait RosyGte<Rhs = Self> {
    type Output;
    fn rosy_gte(self, rhs: Rhs) -> Result<Self::Output>;
}

// RE >= RE
impl RosyGte<&RE> for &RE {
    type Output = LO;
    fn rosy_gte(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self >= rhs)
    }
}

// ST >= ST (lexicographic ordering)
impl RosyGte<&ST> for &ST {
    type Output = LO;
    fn rosy_gte(self, rhs: &ST) -> Result<Self::Output> {
        Ok(self >= rhs)
    }
}
