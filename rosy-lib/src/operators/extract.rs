//! Extraction operator for Rosy types.
//!
//! This module provides the `RosyExtract` trait and implementations for all
//! supported type combinations. The compatibility rules are defined in the
//! `EXTRACT_REGISTRY` constant below.
//!
//! # Type Compatibility
//! 
//! See `assets/operators/extract/extract_table.md` for the full compatibility table.
//!
//! # Examples
//! 
//! See `assets/operators/extract/extract.rosy` for Rosy examples and 
//! `assets/operators/extract/extract.fox` for equivalent COSY INFINITY code.

use anyhow::{Result, bail};

use crate::RosyType;
use crate::{RE, ST, VE, CM, DA, CD};
use std::sync::OnceLock;
use std::collections::HashMap;
use crate::operators::{TypeRule, build_type_registry};
use crate::taylor::monomial::Monomial;

/// Type compatibility registry for extraction operator.
/// 
/// This is the single source of truth for what type combinations are allowed.
/// The build script (`build.rs`) parses this to generate:
/// - Documentation table (`extract_table.md`)
/// - Rosy test script (`extract.rosy`)
/// - COSY test script (`extract.fox`)
/// - Integration tests
/// 
/// This registry matches COSY INFINITY's | operator capabilities exactly,
/// as documented in manual.md Section A.2.
pub const EXTRACT_REGISTRY: &[TypeRule] = &[
    TypeRule::with_comment("ST", "RE", "ST", "'test'", "2", "Extract i-th character"),
    TypeRule::with_comment("ST", "VE", "ST", "'test'", "2&3", "Extract substring by range"),
    TypeRule::with_comment("CM", "RE", "RE", "CM(3&4)", "1", "Extract real part"),
    TypeRule::with_comment("VE", "RE", "RE", "1&2", "2", "Extract i-th component"),
    TypeRule::with_comment("VE", "VE", "VE", "1&2&3", "2&3", "Extract subvector by range"),
    TypeRule::with_comment("DA", "RE", "RE", "DA(1)", "1", "Extract 1D DA coefficient for supplied exponent"),
    TypeRule::with_comment("DA", "VE", "RE", "DA(1)", "0&1", "Extract DA coefficient by exponent vector"),
    TypeRule::with_comment("CD", "RE", "CM", "CD(1)", "1", "Extract 1D CD coefficient for supplied exponent"),
    TypeRule::with_comment("CD", "VE", "CM", "CD(1)", "0&1", "Extract CD coefficient by exponent vector"),
];

static EXTRACT_MAP: OnceLock<HashMap<(RosyType, RosyType), RosyType>> = OnceLock::new();

pub fn get_return_type(base: &RosyType, index: &RosyType) -> Option<RosyType> {
    EXTRACT_MAP.get_or_init(|| build_type_registry(EXTRACT_REGISTRY))
        .get(&(*base, *index))
        .copied()
}

/// Trait for extracting components from Rosy data types
pub trait RosyExtract<T> {
    type Output;
    fn rosy_extract(self, index: T) -> Result<Self::Output>;
}

// ST | RE -> ST (extract i-th character)
impl RosyExtract<&RE> for &ST {
    type Output = ST;
    
    fn rosy_extract(self, index: &RE) -> Result<Self::Output> {
        let idx = index.round() as usize;
        if idx == 0 || idx > self.len() {
            bail!("String index {} out of bounds (1-{})", idx, self.len());
        }
        
        // Rosy uses 1-based indexing
        let char_at_idx = self.chars().nth(idx - 1)
            .ok_or_else(|| anyhow::anyhow!("Character at index {} not found", idx))?;
        
        Ok(char_at_idx.to_string())
    }
}

// ST | VE -> ST (extract substring by range)
impl RosyExtract<&VE> for &ST {
    type Output = ST;
    
    fn rosy_extract(self, index: &VE) -> Result<Self::Output> {
        if index.len() != 2 {
            bail!("String extraction with vector index requires exactly two elements (start and end)");
        }
        
        let start = index[0].round() as usize;
        let end = index[1].round() as usize;

        if start == 0 || end == 0 || start > self.len() || end > self.len() || start > end {
            bail!("String index range {}-{} out of bounds (1-{})", start, end, self.len());
        }
        
        // Rosy uses 1-based indexing
        let substring: String = self.chars().skip(start - 1).take(end - start + 1).collect();
        
        Ok(substring)
    }
}

// CM | RE -> RE (extract real or imaginary part)
impl RosyExtract<&RE> for &CM {
    type Output = RE;
    
    fn rosy_extract(self, index: &RE) -> Result<Self::Output> {
        match *index as i32 {
            1 => Ok(self.re), // Real part
            2 => Ok(self.im), // Imaginary part
            _ => bail!("Complex number index must be 1 (real) or 2 (imaginary), found {}", index),
        }
    }
}

// VE | RE -> RE (extract i-th component)
impl RosyExtract<&RE> for &VE {
    type Output = RE;
    
    fn rosy_extract(self, index: &RE) -> Result<Self::Output> {
        let idx = index.round() as usize;
        if idx == 0 || idx > self.len() {
            bail!("Vector index {} out of bounds (1-{})", idx, self.len());
        }
        
        // Rosy uses 1-based indexing
        Ok(self[idx - 1])
    }
}

// VE | VE -> VE (extract subvector by range)
impl RosyExtract<&VE> for &VE {
    type Output = VE;

    fn rosy_extract(self, index: &VE) -> Result<Self::Output> {
        if index.len() != 2 {
            bail!("Vector extraction with vector index requires exactly two elements (start and end)");
        }
        
        let start = index[0].round() as usize;
        let end = index[1].round() as usize;

        if start == 0 || end == 0 || start > self.len() || end > self.len() || start > end {
            bail!("Vector index range {}-{} out of bounds (1-{})", start, end, self.len());
        }
        
        // Rosy uses 1-based indexing
        Ok(self[start - 1..end].to_vec())
    }
}

// DA | RE -> RE (extract 1D DA coefficient for supplied exponent)
//
// COSY semantics: the RE value is the exponent of the first variable.
// `DA(1) | 1` extracts the coefficient of x1^1 from the DA representing x1,
// which is 1.0.
impl RosyExtract<&RE> for &DA {
    type Output = RE;

    fn rosy_extract(self, index: &RE) -> Result<Self::Output> {
        let exp = *index as u8;
        let mut exponents = [0u8; crate::taylor::MAX_VARS];
        exponents[0] = exp;
        let monomial = Monomial::new(exponents);
        Ok(self.get_coeff(&monomial))
    }
}

// DA | VE -> RE (extract DA coefficient by exponent vector)
impl RosyExtract<&VE> for &DA {
    type Output = RE;

    fn rosy_extract(self, index: &VE) -> Result<Self::Output> {
        let config = crate::taylor::get_config()
            .map_err(|e| anyhow::anyhow!("DA extraction requires initialized Taylor: {}", e))?;
        if index.len() > config.num_vars as usize {
            bail!(
                "Exponent vector length {} exceeds number of DA variables {}",
                index.len(), config.num_vars
            );
        }
        let mut exponents = [0u8; crate::taylor::MAX_VARS];
        for (i, &val) in index.iter().enumerate() {
            exponents[i] = val as u8;
        }
        let monomial = Monomial::new(exponents);
        Ok(self.get_coeff(&monomial))
    }
}

// CD | RE -> CM (extract 1D CD coefficient for supplied exponent)
//
// COSY semantics: the RE value is the exponent of the first variable.
// `CD(1) | 1` extracts the coefficient of x1^1 from the CD representing x1,
// which is (1.0, 0.0).
impl RosyExtract<&RE> for &CD {
    type Output = CM;

    fn rosy_extract(self, index: &RE) -> Result<Self::Output> {
        let exp = *index as u8;
        let mut exponents = [0u8; crate::taylor::MAX_VARS];
        exponents[0] = exp;
        let monomial = Monomial::new(exponents);
        Ok(self.get_coeff(&monomial))
    }
}

// CD | VE -> CM (extract CD coefficient by exponent vector)
impl RosyExtract<&VE> for &CD {
    type Output = CM;

    fn rosy_extract(self, index: &VE) -> Result<Self::Output> {
        let config = crate::taylor::get_config()
            .map_err(|e| anyhow::anyhow!("CD extraction requires initialized Taylor: {}", e))?;
        if index.len() > config.num_vars as usize {
            bail!(
                "Exponent vector length {} exceeds number of CD variables {}",
                index.len(), config.num_vars
            );
        }
        let mut exponents = [0u8; crate::taylor::MAX_VARS];
        for (i, &val) in index.iter().enumerate() {
            exponents[i] = val as u8;
        }
        let monomial = Monomial::new(exponents);
        Ok(self.get_coeff(&monomial))
    }
}
