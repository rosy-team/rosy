//! Extraction operator for Rosy types.

use anyhow::{Result, bail};

use crate::{RosyType, RosyBaseType};
use crate::{RE, ST, VE, CM, DA, CD};
use crate::taylor::monomial::Monomial;

pub fn get_return_type(base: &RosyType, index: &RosyType) -> Option<RosyType> {
    use RosyBaseType::*;
    match crate::operators::dim0(base, index)? {
        (ST, RE) | (ST, VE) => Some(RosyType::ST()),
        (RE, RE) | (RE, VE) => Some(RosyType::RE()),
        (CM, RE) | (VE, RE) | (DA, RE) | (DA, VE) => Some(RosyType::RE()),
        (VE, VE) => Some(RosyType::VE()),
        (CD, RE) | (CD, VE) => Some(RosyType::CM()),
        _ => None,
    }
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

// RE | RE -> RE (COSY sometimes indexes a 0-d cell as a 1-vector)
impl RosyExtract<&RE> for &RE {
    type Output = RE;
    fn rosy_extract(self, index: &RE) -> Result<Self::Output> {
        if index.round() == 1.0 {
            Ok(*self)
        } else {
            bail!("Cannot extract index {} from a scalar", index);
        }
    }
}

impl RosyExtract<&VE> for &RE {
    type Output = RE;
    fn rosy_extract(self, _index: &VE) -> Result<Self::Output> {
        Ok(*self)
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
