//! Not-equals operator for Rosy types.
//!
//! This module provides the `RosyNeq` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `NEQ_REGISTRY` constant below.
//!
//! # Type Compatibility
//!
//! See `assets/operators/neq/neq_table.md` for the full compatibility table.
//!
//! # Examples
//!
//! See `assets/operators/neq/neq.rosy` for Rosy examples and
//! `assets/operators/neq/neq.fox` for equivalent COSY INFINITY code.

use crate::rosy_lib::RosyType;
use crate::rosy_lib::operators::{TypeRule, build_type_registry};
use crate::rosy_lib::{LO, RE, ST};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Type compatibility registry for not-equals operator.
///
/// This is the single source of truth for what type combinations are allowed.
/// The build script (`build.rs`) parses this to generate:
/// - Documentation table (`neq_table.md`)
/// - Rosy test script (`neq.rosy`)
/// - COSY test script (`neq.fox`)
/// - Integration tests
pub const NEQ_REGISTRY: &[TypeRule] = &[
    TypeRule::with_comment(
        "RE",
        "RE",
        "LO",
        "3.14159",
        "2.71828",
        "Exact IEEE-754 not-equals (matches COSY behavior)",
    ),
    TypeRule::with_comment("ST", "ST", "LO", "'hello'", "'world'", "String not-equals"),
    TypeRule::with_comment("LO", "LO", "LO", "TRUE", "FALSE", "Logical not-equals"),
];

static NEQ_MAP: OnceLock<HashMap<(RosyType, RosyType), RosyType>> = OnceLock::new();

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    NEQ_MAP
        .get_or_init(|| build_type_registry(NEQ_REGISTRY))
        .get(&(*lhs, *rhs))
        .copied()
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
