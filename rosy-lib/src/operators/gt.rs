//! Greater-than operator for Rosy types.
//!
//! This module provides the `RosyGt` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `GT_REGISTRY` constant below.

use anyhow::Result;
use crate::RosyType;
use crate::{RE, ST, LO};
use std::sync::OnceLock;
use std::collections::HashMap;
use crate::operators::{TypeRule, build_type_registry};

/// Type compatibility registry for greater-than operator.
pub const GT_REGISTRY: &[TypeRule] = &[
    TypeRule::new("RE", "RE", "LO"),
    TypeRule::new("ST", "ST", "LO"),
];

static GT_MAP: OnceLock<HashMap<(RosyType, RosyType), RosyType>> = OnceLock::new();

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    GT_MAP.get_or_init(|| build_type_registry(GT_REGISTRY))
        .get(&(*lhs, *rhs))
        .copied()
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
