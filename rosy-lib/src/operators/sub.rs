//! Subtraction operator for Rosy types.
//!
//! This module provides the `RosySub` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `SUB_REGISTRY` constant below.
//!
//! # Type Compatibility
//! 
//! See `assets/operators/sub/sub_table.md` for the full compatibility table.
//!
//! # Examples
//! 
//! See `assets/operators/sub/sub.rosy` for Rosy examples and 
//! `assets/operators/sub/sub.fox` for equivalent COSY INFINITY code.

use anyhow::Result;
use num_complex::Complex64;
use crate::RosyType;
use crate::{RE, CM, VE, DA, CD};
use std::sync::OnceLock;
use std::collections::HashMap;
use crate::operators::{TypeRule, build_type_registry};

/// Type compatibility registry for subtraction operator.
/// 
/// This is the single source of truth for what type combinations are allowed.
/// The build script (`build.rs`) parses this to generate:
/// - Documentation table (`sub_table.md`)
/// - Rosy test script (`sub.rosy`)
/// - COSY test script (`sub.fox`)
/// - Integration tests
pub const SUB_REGISTRY: &[TypeRule] = &[
    TypeRule::new("RE", "RE", "RE", "2", "1"),
    TypeRule::new("RE", "CM", "CM", "3", "CM(1&2)"),
    TypeRule::with_comment("RE", "VE", "VE", "5", "1&2", "Subtract componentwise from Real"),
    TypeRule::new("RE", "DA", "DA", "4", "DA(1)"),
    TypeRule::new("RE", "CD", "CD", "5", "DA(1)+CM(1&2)*DA(2)"),
    TypeRule::new("CM", "RE", "CM", "CM(3&4)", "2"),
    TypeRule::new("CM", "CM", "CM", "CM(5&6)", "CM(7&8)"),
    TypeRule::new("CM", "DA", "CD", "CM(1&2)", "DA(1)"),
    TypeRule::new("CM", "CD", "CD", "CM(1&2)", "DA(1)+CM(3&4)*DA(2)"),
    TypeRule::with_comment("VE", "RE", "VE", "3&4", "3", "Subtract Real componentwise"),
    TypeRule::with_comment("VE", "VE", "VE", "5&6", "7&8", "Subtract componentwise"),
    TypeRule::new("DA", "RE", "DA", "DA(1)", "3"),
    TypeRule::new("DA", "CM", "CD", "DA(1)", "CM(1&2)"),
    TypeRule::new("DA", "DA", "DA", "DA(2)", "DA(3)"),
    TypeRule::new("DA", "CD", "CD", "DA(1)", "DA(2)+CM(1&2)*DA(3)"),
    TypeRule::new("CD", "RE", "CD", "DA(1)+CM(1&2)*DA(2)", "4"),
    TypeRule::new("CD", "CM", "CD", "DA(1)+CM(1&2)*DA(2)", "CM(3&4)"),
    TypeRule::new("CD", "DA", "CD", "DA(1)+CM(1&2)*DA(2)", "DA(3)"),
    TypeRule::new("CD", "CD", "CD", "DA(1)+CM(1&2)*DA(2)", "DA(3)+CM(5&6)*DA(4)"),
];

static SUB_MAP: OnceLock<HashMap<(RosyType, RosyType), RosyType>> = OnceLock::new();

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    SUB_MAP.get_or_init(|| build_type_registry(SUB_REGISTRY))
        .get(&(*lhs, *rhs))
        .copied()
}

pub trait RosySub<Rhs = Self> {
    type Output;
    fn rosy_sub(self, rhs: Rhs) -> Result<Self::Output>;
}

// RE - RE
impl RosySub<&RE> for &RE {
    type Output = RE;
    fn rosy_sub(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self - rhs)
    }
}

// RE - CM
impl RosySub<&CM> for &RE {
    type Output = CM;
    fn rosy_sub(self, other: &CM) -> Result<Self::Output> {
        Ok(Complex64::new(self - other.re, -other.im))
    }
}

// RE - VE
impl RosySub<&VE> for &RE {
    type Output = VE;
    fn rosy_sub(self, other: &VE) -> Result<Self::Output> {
        Ok(other.iter().map(|x| self - x).collect())
    }
}

// RE - DA
impl RosySub<&DA> for &RE {
    type Output = DA;
    fn rosy_sub(self, other: &DA) -> Result<Self::Output> {
        &DA::constant(*self) - other
    }
}

// RE - CD
impl RosySub<&CD> for &RE {
    type Output = CD;
    fn rosy_sub(self, other: &CD) -> Result<Self::Output> {
        // Create DA from real, then CD from that DA
        let self_da = DA::constant(*self);
        let self_cd = CD::from_da(&self_da);
        self_cd.rosy_sub(other)
    }
}

// CM - RE
impl RosySub<&RE> for &CM {
    type Output = CM;
    fn rosy_sub(self, other: &RE) -> Result<Self::Output> {
        Ok(Complex64::new(self.re - other, self.im))
    }
}

// CM - CM
impl RosySub<&CM> for &CM {
    type Output = CM;
    fn rosy_sub(self, other: &CM) -> Result<Self::Output> {
        Ok(self - other)
    }
}

// CM - DA
impl RosySub<&DA> for &CM {
    type Output = CD;
    fn rosy_sub(self, other: &DA) -> Result<Self::Output> {
        // Create CD from the complex number
        let cm_cd = CD::complex_constant(*self);
        // Create CD from the DA (which becomes the real part)
        let da_cd = CD::from_da(other);
        // Subtract them
        &cm_cd - &da_cd
    }
}

// CM - CD
impl RosySub<&CD> for &CM {
    type Output = CD;
    fn rosy_sub(self, other: &CD) -> Result<Self::Output> {
        let self_cd = CD::complex_constant(*self);
        &self_cd - other
    }
}

// VE - RE
impl RosySub<&RE> for &VE {
    type Output = VE;
    fn rosy_sub(self, other: &RE) -> Result<Self::Output> {
        Ok(self.iter().map(|x| x - other).collect())
    }
}

// VE - VE
impl RosySub<&VE> for &VE {
    type Output = VE;
    fn rosy_sub(self, other: &VE) -> Result<Self::Output> {
        anyhow::ensure!(self.len() == other.len(),
            "Vector length mismatch in subtraction: {} vs {}", self.len(), other.len());
        Ok(self.iter()
            .zip(other.iter())
            .map(|(x, y)| x - y)
            .collect())
    }
}

// DA - RE
impl RosySub<&RE> for &DA {
    type Output = DA;
    fn rosy_sub(self, other: &RE) -> Result<Self::Output> {
        self - *other
    }
}

// DA - CM
impl RosySub<&CM> for &DA {
    type Output = CD;
    fn rosy_sub(self, other: &CM) -> Result<Self::Output> {
        // Create CD from the complex number
        let cm_cd = CD::complex_constant(*other);
        // Create CD from the DA (which becomes the real part)
        let da_cd = CD::from_da(self);
        // Subtract them
        &da_cd - &cm_cd
    }
}

// DA - DA
impl RosySub<&DA> for &DA {
    type Output = DA;
    fn rosy_sub(self, other: &DA) -> Result<Self::Output> {
        self - other
    }
}

// DA - CD
impl RosySub<&CD> for &DA {
    type Output = CD;
    fn rosy_sub(self, other: &CD) -> Result<Self::Output> {
        // Create CD from DA (real part only, imaginary is zero)
        let self_cd = CD::from_da(self);
        &self_cd - other
    }
}

// CD - RE
impl RosySub<&RE> for &CD {
    type Output = CD;
    fn rosy_sub(self, other: &RE) -> Result<Self::Output> {
        use num_complex::Complex64;
        self - Complex64::new(*other, 0.0)
    }
}

// CD - CM
impl RosySub<&CM> for &CD {
    type Output = CD;
    fn rosy_sub(self, other: &CM) -> Result<Self::Output> {
        use num_complex::Complex64;
        self - *other
    }
}

// CD - DA
impl RosySub<&DA> for &CD {
    type Output = CD;
    fn rosy_sub(self, other: &DA) -> Result<Self::Output> {
        // Create CD from DA (real part only, imaginary is zero)
        let other_cd = CD::from_da(other);
        self - &other_cd
    }
}

// CD - CD
impl RosySub<&CD> for &CD {
    type Output = CD;
    fn rosy_sub(self, other: &CD) -> Result<Self::Output> {
        self - other
    }
}

