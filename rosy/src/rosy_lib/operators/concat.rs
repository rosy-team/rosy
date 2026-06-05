//! Concatenation operator for Rosy types.
//!
//! This module provides the `RosyConcat` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `CONCAT_REGISTRY` constant below.
//!
//! # Type Compatibility
//! 
//! See `assets/operators/concat/concat_table.md` for the full compatibility table.
//!
//! # Examples
//! 
//! See `assets/operators/concat/concat.rosy` for Rosy examples and 
//! `assets/operators/concat/concat.fox` for equivalent COSY INFINITY code.

use anyhow::Result;
use crate::rosy_lib::RosyType;
use crate::rosy_lib::{RE, ST, VE, DA, CD};
use std::sync::OnceLock;
use std::collections::HashMap;
use crate::rosy_lib::operators::{TypeRule, build_type_registry};

/// Type compatibility registry for concatenation operator.
/// 
/// This is the single source of truth for what type combinations are allowed.
/// The build script (`build.rs`) parses this to generate:
/// - Documentation table (`concat_table.md`)
/// - Rosy test script (`concat.rosy`)
/// - COSY test script (`concat.fox`)
/// - Integration tests
/// 
/// **Note:** This registry matches COSY INFINITY's & operator capabilities.
/// See manual.md Section A.2 "& (Concatenation)" for the authoritative list.
/// GR (Graphics) type is not yet implemented in Rosy.
pub const CONCAT_REGISTRY: &[TypeRule] = &[
    TypeRule::with_comment("RE", "RE", "VE", "1", "1", "Concatenate two Reals to a Vector"),
    TypeRule::with_comment("RE", "VE", "VE", "1", "1&2&3", "Prepend a Real to the left of a Vector"),
    TypeRule::with_comment("ST", "ST", "ST", "'He'", "'ya!'", "Concatenate two Strings"),
    TypeRule::with_comment("VE", "RE", "VE", "1&2", "3", "Append a Real to the right of a Vector"),
    TypeRule::with_comment("VE", "VE", "VE", "1&2", "3&4", "Concatenate two Vectors"),
    // DA concatenation — builds vectors of Taylor series (phase-space maps)
    TypeRule::with_comment("DA", "DA", "DA1", "DA(1)", "DA(2)", "Concatenate two DAs to a DA vector"),
    TypeRule::with_comment("DA", "DA1", "DA1", "DA(1)", "DA(1)&DA(2)", "Prepend a DA to the left of a DA vector"),
    TypeRule::with_comment("DA1", "DA", "DA1", "DA(1)&DA(2)", "DA(3)", "Append a DA to the right of a DA vector"),
    TypeRule::with_comment("DA1", "DA1", "DA1", "DA(1)&DA(2)", "DA(3)&DA(4)", "Concatenate two DA vectors"),
    // CD concatenation — builds vectors of complex Taylor series
    TypeRule::with_comment("CD", "CD", "CD1", "CD(1)", "CD(2)", "Concatenate two CDs to a CD vector"),
    TypeRule::with_comment("CD", "CD1", "CD1", "CD(1)", "CD(1)&CD(2)", "Prepend a CD to the left of a CD vector"),
    TypeRule::with_comment("CD1", "CD", "CD1", "CD(1)&CD(2)", "CD(3)", "Append a CD to the right of a CD vector"),
    TypeRule::with_comment("CD1", "CD1", "CD1", "CD(1)&CD(2)", "CD(3)&CD(4)", "Concatenate two CD vectors"),
    // GR & GR => GR is in COSY but GR type not implemented in Rosy yet
];

static CONCAT_MAP: OnceLock<HashMap<(RosyType, RosyType), RosyType>> = OnceLock::new();

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    CONCAT_MAP.get_or_init(|| build_type_registry(CONCAT_REGISTRY))
        .get(&(*lhs, *rhs))
        .copied()
}

pub trait RosyConcat<Rhs = Self> {
    type Output;
    fn rosy_concat(self, rhs: Rhs) -> Result<Self::Output>;
}

// RE & RE => VE
impl RosyConcat<&RE> for &RE {
    type Output = VE;
    fn rosy_concat(self, other: &RE) -> Result<Self::Output> {
        Ok(vec![*self, *other])
    }
}

// RE & VE => VE
impl RosyConcat<&VE> for &RE {
    type Output = VE;
    fn rosy_concat(self, other: &VE) -> Result<Self::Output> {
        let mut result = vec![*self];
        result.extend_from_slice(other);
        Ok(result)
    }
}

// ST & ST => ST
impl RosyConcat<&ST> for &ST {
    type Output = ST;
    fn rosy_concat(self, other: &ST) -> Result<Self::Output> {
        Ok(format!("{}{}", self, other))
    }
}

// VE & RE => VE
impl RosyConcat<&RE> for &VE {
    type Output = VE;
    fn rosy_concat(self, other: &RE) -> Result<Self::Output> {
        let mut result = self.clone();
        result.push(*other);
        Ok(result)
    }
}

// VE & VE => VE
impl RosyConcat<&VE> for &VE {
    type Output = VE;
    fn rosy_concat(self, other: &VE) -> Result<Self::Output> {
        let mut result = self.clone();
        result.extend_from_slice(other);
        Ok(result)
    }
}

// DA & DA => Vec<DA>
impl RosyConcat<&DA> for &DA {
    type Output = Vec<DA>;
    fn rosy_concat(self, other: &DA) -> Result<Self::Output> {
        Ok(vec![self.clone(), other.clone()])
    }
}

// DA & Vec<DA> => Vec<DA>
impl RosyConcat<&Vec<DA>> for &DA {
    type Output = Vec<DA>;
    fn rosy_concat(self, other: &Vec<DA>) -> Result<Self::Output> {
        let mut result = vec![self.clone()];
        result.extend_from_slice(other);
        Ok(result)
    }
}

// Vec<DA> & DA => Vec<DA>
impl RosyConcat<&DA> for &Vec<DA> {
    type Output = Vec<DA>;
    fn rosy_concat(self, other: &DA) -> Result<Self::Output> {
        let mut result = self.clone();
        result.push(other.clone());
        Ok(result)
    }
}

// Vec<DA> & Vec<DA> => Vec<DA>
impl RosyConcat<&Vec<DA>> for &Vec<DA> {
    type Output = Vec<DA>;
    fn rosy_concat(self, other: &Vec<DA>) -> Result<Self::Output> {
        let mut result = self.clone();
        result.extend_from_slice(other);
        Ok(result)
    }
}

// CD & CD => Vec<CD>
impl RosyConcat<&CD> for &CD {
    type Output = Vec<CD>;
    fn rosy_concat(self, other: &CD) -> Result<Self::Output> {
        Ok(vec![self.clone(), other.clone()])
    }
}

// CD & Vec<CD> => Vec<CD>
impl RosyConcat<&Vec<CD>> for &CD {
    type Output = Vec<CD>;
    fn rosy_concat(self, other: &Vec<CD>) -> Result<Self::Output> {
        let mut result = vec![self.clone()];
        result.extend_from_slice(other);
        Ok(result)
    }
}

// Vec<CD> & CD => Vec<CD>
impl RosyConcat<&CD> for &Vec<CD> {
    type Output = Vec<CD>;
    fn rosy_concat(self, other: &CD) -> Result<Self::Output> {
        let mut result = self.clone();
        result.push(other.clone());
        Ok(result)
    }
}

// Vec<CD> & Vec<CD> => Vec<CD>
impl RosyConcat<&Vec<CD>> for &Vec<CD> {
    type Output = Vec<CD>;
    fn rosy_concat(self, other: &Vec<CD>) -> Result<Self::Output> {
        let mut result = self.clone();
        result.extend_from_slice(other);
        Ok(result)
    }
}
