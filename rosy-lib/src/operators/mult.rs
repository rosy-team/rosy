//! Multiplication operator for Rosy types.

use anyhow::Result;
use crate::RosyType;
use crate::{RE, CM, VE, DA, CD, LO};

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    crate::operators::arith_return(lhs, rhs, true)
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

// VE * DA: COSY 1-vector cell times a series
impl RosyMult<&DA> for &VE {
    type Output = DA;
    fn rosy_mult(self, other: &DA) -> Result<Self::Output> {
        anyhow::ensure!(
            self.len() == 1,
            "VE*DA needs length 1, got {}",
            self.len()
        );
        other * self[0]
    }
}

// DA * VE
impl RosyMult<&VE> for &DA {
    type Output = DA;
    fn rosy_mult(self, other: &VE) -> Result<Self::Output> {
        anyhow::ensure!(
            other.len() == 1,
            "DA*VE needs length 1, got {}",
            other.len()
        );
        self * other[0]
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

