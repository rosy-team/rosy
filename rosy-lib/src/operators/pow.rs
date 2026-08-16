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
use crate::{RosyType, RosyBaseType};
use crate::{RE, VE, DA, CD};
use crate::core::polval::{da_powi, cd_powi};

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    match crate::operators::dim0(lhs, rhs)? {
        (RosyBaseType::RE, RosyBaseType::RE) => Some(RosyType::RE()),
        (RosyBaseType::VE, RosyBaseType::RE) => Some(RosyType::VE()),
        (RosyBaseType::DA, RosyBaseType::RE) => Some(RosyType::DA()),
        (RosyBaseType::CD, RosyBaseType::RE) => Some(RosyType::CD()),
        _ => None,
    }
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

