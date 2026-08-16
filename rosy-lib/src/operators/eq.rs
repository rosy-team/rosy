//! Equality operator for Rosy types.
//!
//! This module provides the `RosyEq` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `EQ_REGISTRY` constant below.
//!
//! # Type Compatibility
//! 
//! See `assets/operators/eq/eq_table.md` for the full compatibility table.
//!
//! # Examples
//! 
//! See `assets/operators/eq/eq.rosy` for Rosy examples and 
//! `assets/operators/eq/eq.fox` for equivalent COSY INFINITY code.

use anyhow::Result;
use crate::RosyType;
use crate::{RE, ST, LO};
use std::sync::OnceLock;
use std::collections::HashMap;
use crate::operators::{TypeRule, build_type_registry};

/// Type compatibility registry for equality operator.
/// 
/// This is the single source of truth for what type combinations are allowed.
/// The build script (`build.rs`) parses this to generate:
/// - Documentation table (`eq_table.md`)
/// - Rosy test script (`eq.rosy`)
/// - COSY test script (`eq.fox`)
/// - Integration tests
pub const EQ_REGISTRY: &[TypeRule] = &[
    TypeRule::new("RE", "RE", "LO"),
    TypeRule::new("ST", "ST", "LO"),
    TypeRule::new("LO", "LO", "LO"),
];

static EQ_MAP: OnceLock<HashMap<(RosyType, RosyType), RosyType>> = OnceLock::new();

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    EQ_MAP.get_or_init(|| build_type_registry(EQ_REGISTRY))
        .get(&(*lhs, *rhs))
        .copied()
}

pub trait RosyEq<Rhs = Self> {
    type Output;
    fn rosy_eq(self, rhs: Rhs) -> Result<Self::Output>;
}

// RE = RE (exact IEEE-754, matches COSY behavior)
impl RosyEq<&RE> for &RE {
    type Output = LO;
    fn rosy_eq(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self == rhs)
    }
}

// ST = ST (exact string equality)
impl RosyEq<&ST> for &ST {
    type Output = LO;
    fn rosy_eq(self, rhs: &ST) -> Result<Self::Output> {
        Ok(self == rhs)
    }
}

// LO = LO (logical equality)
impl RosyEq<&LO> for &LO {
    type Output = LO;
    fn rosy_eq(self, rhs: &LO) -> Result<Self::Output> {
        Ok(self == rhs)
    }
}
