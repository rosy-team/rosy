//! Division operator for Rosy types.
//!
//! This module provides the `RosyDiv` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `DIV_REGISTRY` constant below.
//!
//! # Type Compatibility
//! 
//! See `assets/operators/div/div_table.md` for the full compatibility table.
//!
//! # Examples
//! 
//! See `assets/operators/div/div.rosy` for Rosy examples and 
//! `assets/operators/div/div.fox` for equivalent COSY INFINITY code.

use anyhow::Result;
use crate::RosyType;
use crate::{RE, CM, VE, DA, CD};
use std::sync::OnceLock;
use std::collections::HashMap;
use crate::operators::{TypeRule, build_type_registry};

/// Type compatibility registry for division operator.
/// 
/// This is the single source of truth for what type combinations are allowed.
/// The build script (`build.rs`) parses this to generate:
/// - Documentation table (`div_table.md`)
/// - Rosy test script (`div.rosy`)
/// - COSY test script (`div.fox`)
/// - Integration tests
pub const DIV_REGISTRY: &[TypeRule] = &[
    TypeRule::new("RE", "RE", "RE"),
    TypeRule::new("RE", "CM", "CM"),
    TypeRule::new("RE", "VE", "VE"),
    TypeRule::new("RE", "DA", "DA"),
    TypeRule::new("RE", "CD", "CD"),
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

static DIV_MAP: OnceLock<HashMap<(RosyType, RosyType), RosyType>> = OnceLock::new();

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    DIV_MAP.get_or_init(|| build_type_registry(DIV_REGISTRY))
        .get(&(*lhs, *rhs))
        .copied()
}

pub trait RosyDiv<Rhs = Self> {
    type Output;
    fn rosy_div(self, rhs: Rhs) -> Result<Self::Output>;
}

// RE / RE
impl RosyDiv<&RE> for &RE {
    type Output = RE;
    fn rosy_div(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self / rhs)
    }
}

// RE / CM
impl RosyDiv<&CM> for &RE {
    type Output = CM;
    fn rosy_div(self, other: &CM) -> Result<Self::Output> {
        // Division by complex: a / (b + ci) = a(b - ci) / (b^2 + c^2)
        Ok(self / other)
    }
}

// RE / VE
impl RosyDiv<&VE> for &RE {
    type Output = VE;
    fn rosy_div(self, other: &VE) -> Result<Self::Output> {
        Ok(other.iter().map(|x| self / x).collect())
    }
}

// RE / DA
impl RosyDiv<&DA> for &RE {
    type Output = DA;
    fn rosy_div(self, other: &DA) -> Result<Self::Output> {
        &DA::constant(*self) / other
    }
}

// RE / CD
impl RosyDiv<&CD> for &RE {
    type Output = CD;
    fn rosy_div(self, other: &CD) -> Result<Self::Output> {
        // Create DA from real, then CD from that DA
        let self_da = DA::constant(*self);
        let self_cd = CD::from_da(&self_da);
        self_cd.rosy_div(other)
    }
}

// CM / RE
impl RosyDiv<&RE> for &CM {
    type Output = CM;
    fn rosy_div(self, other: &RE) -> Result<Self::Output> {
        Ok(self / other)
    }
}

// CM / CM
impl RosyDiv<&CM> for &CM {
    type Output = CM;
    fn rosy_div(self, other: &CM) -> Result<Self::Output> {
        // (a + bi) / (c + di) = ((ac + bd) + (bc - ad)i) / (c^2 + d^2)
        Ok(self / other)
    }
}

// CM / DA
impl RosyDiv<&DA> for &CM {
    type Output = CD;
    fn rosy_div(self, other: &DA) -> Result<Self::Output> {
        // Create CD from the complex number
        let cm_cd = CD::complex_constant(*self);
        // Create CD from the DA (which becomes the real part)
        let da_cd = CD::from_da(other);
        // Divide them
        &cm_cd / &da_cd
    }
}

// CM / CD
impl RosyDiv<&CD> for &CM {
    type Output = CD;
    fn rosy_div(self, other: &CD) -> Result<Self::Output> {
        let self_cd = CD::complex_constant(*self);
        &self_cd / other
    }
}

// VE / RE
impl RosyDiv<&RE> for &VE {
    type Output = VE;
    fn rosy_div(self, other: &RE) -> Result<Self::Output> {
        Ok(self.iter().map(|x| x / other).collect())
    }
}

// VE / VE
impl RosyDiv<&VE> for &VE {
    type Output = VE;
    fn rosy_div(self, other: &VE) -> Result<Self::Output> {
        anyhow::ensure!(self.len() == other.len(),
            "Vector length mismatch in division: {} vs {}", self.len(), other.len());
        Ok(self.iter()
            .zip(other.iter())
            .map(|(x, y)| x / y)
            .collect())
    }
}

// DA / RE
impl RosyDiv<&RE> for &DA {
    type Output = DA;
    fn rosy_div(self, other: &RE) -> Result<Self::Output> {
        self / *other
    }
}

// DA / CM
impl RosyDiv<&CM> for &DA {
    type Output = CD;
    fn rosy_div(self, other: &CM) -> Result<Self::Output> {
        // Create CD from the complex number
        let cm_cd = CD::complex_constant(*other);
        // Create CD from the DA (which becomes the real part)
        let da_cd = CD::from_da(self);
        // Divide them
        &da_cd / &cm_cd
    }
}

// DA / DA
impl RosyDiv<&DA> for &DA {
    type Output = DA;
    fn rosy_div(self, other: &DA) -> Result<Self::Output> {
        self / other
    }
}

// DA / CD
impl RosyDiv<&CD> for &DA {
    type Output = CD;
    fn rosy_div(self, other: &CD) -> Result<Self::Output> {
        // Create CD from DA (real part only, imaginary is zero)
        let self_cd = CD::from_da(self);
        &self_cd / other
    }
}

// CD / RE
impl RosyDiv<&RE> for &CD {
    type Output = CD;
    fn rosy_div(self, other: &RE) -> Result<Self::Output> {
        use num_complex::Complex64;
        self / Complex64::new(*other, 0.0)
    }
}

// CD / CM
impl RosyDiv<&CM> for &CD {
    type Output = CD;
    fn rosy_div(self, other: &CM) -> Result<Self::Output> {
        
        self / *other
    }
}

// CD / DA
impl RosyDiv<&DA> for &CD {
    type Output = CD;
    fn rosy_div(self, other: &DA) -> Result<Self::Output> {
        // Create CD from DA (real part only, imaginary is zero)
        let other_cd = CD::from_da(other);
        self / &other_cd
    }
}

// CD / CD
impl RosyDiv<&CD> for &CD {
    type Output = CD;
    fn rosy_div(self, other: &CD) -> Result<Self::Output> {
        self / other
    }
}
