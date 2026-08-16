//! Greater-than operator for Rosy types.
//!
//! This module provides the `RosyGt` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `GT_REGISTRY` constant below.

use anyhow::Result;
use crate::RosyType;
use crate::{RE, ST, LO};

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    crate::operators::lt::get_return_type(lhs, rhs)
}

pub trait RosyGt<Rhs = Self> {
    type Output;
    fn rosy_gt(self, rhs: Rhs) -> Result<Self::Output>;
}

// RE > RE
impl RosyGt<&RE> for &RE {
    type Output = LO;
    fn rosy_gt(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self > rhs)
    }
}

// ST > ST (lexicographic ordering)
impl RosyGt<&ST> for &ST {
    type Output = LO;
    fn rosy_gt(self, rhs: &ST) -> Result<Self::Output> {
        Ok(self > rhs)
    }
}
