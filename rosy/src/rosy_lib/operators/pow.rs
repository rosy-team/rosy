//! Power/exponentiation operator for Rosy types.
//!
//! This module provides the `RosyPow` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `POW_REGISTRY` constant below.
//!
//! # Type Compatibility
//! 
//! According to COSY INFINITY manual:
//! - RE ^ RE -> RE
//! - VE ^ RE -> VE (component-wise)

use anyhow::Result;
use crate::rosy_lib::RosyType;
use crate::rosy_lib::{RE, VE, DA, CD};
use crate::rosy_lib::operators::{TypeRule, build_type_registry};
use crate::rosy_lib::core::polval::{da_powi, cd_powi};

/// Type compatibility registry for power/exponentiation operator.
///
/// This is the single source of truth for what type combinations are allowed.
pub const POW_REGISTRY: &[TypeRule] = &[
    TypeRule::new("RE", "RE", "RE", "2", "3"),
    TypeRule::with_comment("VE", "RE", "VE", "1&2&3", "2", "Raise to Real power componentwise"),
    TypeRule::new("DA", "RE", "DA", "DA(1)", "2"),
    TypeRule::new("CD", "RE", "CD", "CD(1)", "2"),
];

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    let registry = build_type_registry(POW_REGISTRY);
    registry.get(&(*lhs, *rhs)).copied()
}

pub trait RosyPow<Rhs = Self> {
    type Output;
    fn rosy_pow(self, rhs: Rhs) -> Result<Self::Output>;
}

// RE ^ RE
impl RosyPow<&RE> for &RE {
    type Output = RE;
    fn rosy_pow(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self.powf(*rhs))
    }
}

// VE ^ RE (componentwise)
impl RosyPow<&RE> for &VE {
    type Output = VE;
    fn rosy_pow(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self.iter().map(|x| x.powf(*rhs)).collect())
    }
}

// DA ^ RE (repeated squaring via da_powi; truncates exponent to u8)
impl RosyPow<&RE> for &DA {
    type Output = DA;
    fn rosy_pow(self, rhs: &RE) -> Result<Self::Output> {
        da_powi(self, *rhs as u8)
    }
}

// CD ^ RE (repeated squaring via cd_powi; truncates exponent to u8)
impl RosyPow<&RE> for &CD {
    type Output = CD;
    fn rosy_pow(self, rhs: &RE) -> Result<Self::Output> {
        cd_powi(self, *rhs as u8)
    }
}

