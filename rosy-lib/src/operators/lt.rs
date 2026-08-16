//! Less-than operator for Rosy types.
//!
//! This module provides the `RosyLt` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `LT_REGISTRY` constant below.

use anyhow::Result;
use crate::{RosyType, RosyBaseType};
use crate::{RE, ST, LO};

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    match crate::operators::dim0(lhs, rhs)? {
        (RosyBaseType::RE, RosyBaseType::RE) | (RosyBaseType::ST, RosyBaseType::ST) => {
            Some(RosyType::LO())
        }
        _ => None,
    }
}

pub trait RosyLt<Rhs = Self> {
    type Output;
    fn rosy_lt(self, rhs: Rhs) -> Result<Self::Output>;
}

// RE < RE
impl RosyLt<&RE> for &RE {
    type Output = LO;
    fn rosy_lt(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self < rhs)
    }
}

// ST < ST (lexicographic ordering)
impl RosyLt<&ST> for &ST {
    type Output = LO;
    fn rosy_lt(self, rhs: &ST) -> Result<Self::Output> {
        Ok(self < rhs)
    }
}
