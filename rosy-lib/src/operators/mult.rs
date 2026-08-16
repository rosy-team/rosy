//! Multiplication operator for Rosy types.
//!
//! This module provides the `RosyMult` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `MULT_REGISTRY` constant below.
//!
//! # Type Compatibility
//! 
//! See `assets/operators/mult/mult_table.md` for the full compatibility table.
//!
//! # Examples
//! 
//! See `assets/operators/mult/mult.rosy` for Rosy examples and 
//! `assets/operators/mult/mult.fox` for equivalent COSY INFINITY code.

use anyhow::Result;
use crate::RosyType;
use crate::{RE, CM, VE, DA, CD, LO};
use std::sync::OnceLock;
use std::collections::HashMap;
use crate::operators::{TypeRule, build_type_registry};

/// Type compatibility registry for multiplication operator.
/// 
/// This is the single source of truth for what type combinations are allowed.
/// The build script (`build.rs`) parses this to generate:
/// - Documentation table (`mult_table.md`)
/// - Rosy test script (`mult.rosy`)
/// - COSY test script (`mult.fox`)
/// - Integration tests
pub const MULT_REGISTRY: &[TypeRule] = &[
    TypeRule::new("RE", "RE", "RE"),
    TypeRule::new("RE", "CM", "CM"),
    TypeRule::new("RE", "VE", "VE"),
    TypeRule::new("RE", "DA", "DA"),
    TypeRule::new("RE", "CD", "CD"),
    TypeRule::new("LO", "LO", "LO"),
    TypeRule::new("CM", "RE", "CM"),
    TypeRule::new("CM", "CM", "CM"),
    TypeRule::new("CM", "DA", "CD"),
    TypeRule::new("CM", "CD", "CD"),
    TypeRule::new("VE", "RE", "VE"),
    TypeRule::new("VE", "VE", "VE"),
    TypeRule::new("DA", "RE", "DA"),
    TypeRule::new("DA", "CM", "CD"),
    TypeRule::new("DA", "DA", "DA"),
    TypeRule::new("DA", "CD", "CD"),
    TypeRule::new("CD", "RE", "CD"),
    TypeRule::new("CD", "CM", "CD"),
    TypeRule::new("CD", "DA", "CD"),
    TypeRule::new("CD", "CD", "CD"),
];

static MULT_MAP: OnceLock<HashMap<(RosyType, RosyType), RosyType>> = OnceLock::new();

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    MULT_MAP.get_or_init(|| build_type_registry(MULT_REGISTRY))
        .get(&(*lhs, *rhs))
        .copied()
}

pub trait RosyMult<Rhs = Self> {
    type Output;
    fn rosy_mult(self, rhs: Rhs) -> Result<Self::Output>;
}

// RE * RE
impl RosyMult<&RE> for &RE {
    type Output = RE;
    fn rosy_mult(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self * rhs)
    }
}

// RE * CM
impl RosyMult<&CM> for &RE {
    type Output = CM;
    fn rosy_mult(self, other: &CM) -> Result<Self::Output> {
        Ok(self * other)
    }
}

// RE * VE
impl RosyMult<&VE> for &RE {
    type Output = VE;
    fn rosy_mult(self, other: &VE) -> Result<Self::Output> {
        Ok(other.iter().map(|x| x * self).collect())
    }
}

// RE * DA
impl RosyMult<&DA> for &RE {
    type Output = DA;
    fn rosy_mult(self, other: &DA) -> Result<Self::Output> {
        other * *self
    }
}

// CM * RE
impl RosyMult<&RE> for &CM {
    type Output = CM;
    fn rosy_mult(self, other: &RE) -> Result<Self::Output> {
        Ok(self * other)
    }
}

// CM * CM
impl RosyMult<&CM> for &CM {
    type Output = CM;
    fn rosy_mult(self, other: &CM) -> Result<Self::Output> {
        // (a + bi)(c + di) = (ac - bd) + (ad + bc)i
        Ok(self * other)
    }
}

// VE * RE
impl RosyMult<&RE> for &VE {
    type Output = VE;
    fn rosy_mult(self, other: &RE) -> Result<Self::Output> {
        Ok(self.iter().map(|x| x * other).collect())
    }
}

// VE * VE
impl RosyMult<&VE> for &VE {
    type Output = VE;
    fn rosy_mult(self, other: &VE) -> Result<Self::Output> {
        anyhow::ensure!(self.len() == other.len(),
            "Vector length mismatch in multiplication: {} vs {}", self.len(), other.len());
        Ok(self.iter()
            .zip(other.iter())
            .map(|(x, y)| x * y)
            .collect())
    }
}

// DA * RE
impl RosyMult<&RE> for &DA {
    type Output = DA;
    fn rosy_mult(self, other: &RE) -> Result<Self::Output> {
        self * *other
    }
}

// DA * DA
impl RosyMult<&DA> for &DA {
    type Output = DA;
    fn rosy_mult(self, other: &DA) -> Result<Self::Output> {
        self * other
    }
}

// RE * CD
impl RosyMult<&CD> for &RE {
    type Output = CD;
    fn rosy_mult(self, other: &CD) -> Result<Self::Output> {
        // Create DA from real, then CD from that DA
        let self_da = DA::constant(*self);
        let self_cd = CD::from_da(&self_da);
        self_cd.rosy_mult(other)
    }
}

// LO * LO (Logical AND)
impl RosyMult<&LO> for &LO {
    type Output = LO;
    fn rosy_mult(self, other: &LO) -> Result<Self::Output> {
        Ok(*self && *other)
    }
}

// CM * DA
impl RosyMult<&DA> for &CM {
    type Output = CD;
    fn rosy_mult(self, other: &DA) -> Result<Self::Output> {
        // Create CD from the complex number
        let cm_cd = CD::complex_constant(*self);
        // Create CD from the DA (which becomes the real part)
        let da_cd = CD::from_da(other);
        // Multiply them
        &cm_cd * &da_cd
    }
}

// CM * CD
impl RosyMult<&CD> for &CM {
    type Output = CD;
    fn rosy_mult(self, other: &CD) -> Result<Self::Output> {
        other * *self
    }
}

// DA * CM
impl RosyMult<&CM> for &DA {
    type Output = CD;
    fn rosy_mult(self, other: &CM) -> Result<Self::Output> {
        // Create CD from the complex number
        let cm_cd = CD::complex_constant(*other);
        // Create CD from the DA (which becomes the real part)
        let da_cd = CD::from_da(self);
        // Multiply them
        &da_cd * &cm_cd
    }
}

// DA * CD
impl RosyMult<&CD> for &DA {
    type Output = CD;
    fn rosy_mult(self, other: &CD) -> Result<Self::Output> {
        // Create CD from DA (real part only, imaginary is zero)
        let self_cd = CD::from_da(self);
        &self_cd * other
    }
}

// CD * RE
impl RosyMult<&RE> for &CD {
    type Output = CD;
    fn rosy_mult(self, other: &RE) -> Result<Self::Output> {
        use num_complex::Complex64;
        self * Complex64::new(*other, 0.0)
    }
}

// CD * CM
impl RosyMult<&CM> for &CD {
    type Output = CD;
    fn rosy_mult(self, other: &CM) -> Result<Self::Output> {
        
        self * *other
    }
}

// CD * DA
impl RosyMult<&DA> for &CD {
    type Output = CD;
    fn rosy_mult(self, other: &DA) -> Result<Self::Output> {
        // Create CD from DA (real part only, imaginary is zero)
        let other_cd = CD::from_da(other);
        self * &other_cd
    }
}

// CD * CD
impl RosyMult<&CD> for &CD {
    type Output = CD;
    fn rosy_mult(self, other: &CD) -> Result<Self::Output> {
        self * other
    }
}

