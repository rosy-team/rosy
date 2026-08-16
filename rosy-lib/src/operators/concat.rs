//! Concatenation operator for Rosy types.

use anyhow::Result;
use crate::{RosyType, RosyBaseType};
use crate::{RE, ST, VE, DA, CD};

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    use RosyBaseType::*;
    match (lhs.base_type, lhs.dimensions, rhs.base_type, rhs.dimensions) {
        (RE, 0, RE, 0) | (RE, 0, VE, 0) | (VE, 0, RE, 0) | (VE, 0, VE, 0) => Some(RosyType::VE()),
        (ST, 0, ST, 0) => Some(RosyType::ST()),
        (DA, 0 | 1, DA, 0 | 1) => Some(RosyType::new(DA, 1)),
        (CD, 0 | 1, CD, 0 | 1) => Some(RosyType::new(CD, 1)),
        _ => None,
    }
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
