//! Subtraction operator for Rosy types.

use anyhow::Result;
use num_complex::Complex64;
use crate::RosyType;
use crate::{RE, CM, VE, DA, CD};

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    crate::operators::arith_return(lhs, rhs, false)
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

