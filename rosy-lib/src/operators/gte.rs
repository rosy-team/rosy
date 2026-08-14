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
use std::sync::OnceLock;
use std::collections::HashMap;
use crate::operators::{TypeRule, build_type_registry};

/// Type compatibility registry for greater-than-or-equal operator.
pub const GTE_REGISTRY: &[TypeRule] = &[
    TypeRule::with_comment("RE", "RE", "LO", "2.0", "2.0", "Numeric greater-than-or-equal"),
    TypeRule::with_comment("ST", "ST", "LO", "'banana'", "'banana'", "Lexicographic ordering"),
];

static GTE_MAP: OnceLock<HashMap<(RosyType, RosyType), RosyType>> = OnceLock::new();

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    GTE_MAP.get_or_init(|| build_type_registry(GTE_REGISTRY))
        .get(&(*lhs, *rhs))
        .copied()
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
