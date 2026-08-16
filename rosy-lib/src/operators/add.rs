//! Addition operator for Rosy types.

use anyhow::Result;
use num_complex::Complex64;
use crate::RosyType;
use crate::{RE, CM, VE, DA, CD, LO};

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    crate::operators::arith_return(lhs, rhs, true)
}

pub trait RosyAdd<Rhs = Self> {
    type Output;
    fn rosy_add(self, rhs: Rhs) -> Result<Self::Output>;
}
// RE + RE
impl RosyAdd<&RE> for &RE {
    type Output = RE;
    fn rosy_add(self, rhs: &RE) -> Result<Self::Output> {
        Ok(self + rhs)
    }
}
// RE + CM
impl RosyAdd<&CM> for &RE {
    type Output = CM;
    fn rosy_add(self, other: &CM) -> Result<Self::Output> {
        Ok(CM::new(self + other.re, other.im))
    }
}
// RE + VE
impl RosyAdd<&VE> for &RE {
    type Output = VE;
    fn rosy_add(self, other: &VE) -> Result<Self::Output> {
        Ok(other.iter().map(|x| x + self).collect())
    }
}

// RE + DA
impl RosyAdd<&DA> for &RE {
    type Output = DA;
    fn rosy_add(self, other: &DA) -> Result<Self::Output> {
        other + *self
    }
}

// CM + RE
impl RosyAdd<&RE> for &CM {
    type Output = CM;
    fn rosy_add(self, other: &RE) -> Result<Self::Output> {
        Ok(CM::new(self.re + other, self.im))
    }
}
// CM + CM
impl RosyAdd<&CM> for &CM {
    type Output = CM;
    fn rosy_add(self, other: &CM) -> Result<Self::Output> {
        Ok(self + other)
    }
}

// VE + RE
impl RosyAdd<&RE> for &VE {
    type Output = VE;
    fn rosy_add(self, other: &RE) -> Result<Self::Output> {
        Ok(self.iter().map(|x| x + other).collect())
    }
}
// VE + VE
impl RosyAdd<&VE> for &VE {
    type Output = VE;
    fn rosy_add(self, other: &VE) -> Result<Self::Output> {
        anyhow::ensure!(self.len() == other.len(),
            "Vector length mismatch in addition: {} vs {}", self.len(), other.len());
        Ok(self.iter()
            .zip(other.iter())
            .map(|(x, y)| x + y)
            .collect())
    }
}

// DA + RE
impl RosyAdd<&RE> for &DA {
    type Output = DA;
    fn rosy_add(self, other: &RE) -> Result<Self::Output> {
        self + *other
    }
}

// DA + DA
impl RosyAdd<&DA> for &DA {
    type Output = DA;
    fn rosy_add(self, other: &DA) -> Result<Self::Output> {
        self + other
    }
}

// RE + CD
impl RosyAdd<&CD> for &RE {
    type Output = CD;
    fn rosy_add(self, other: &CD) -> Result<Self::Output> {
        // Create DA from real, then CD from that DA
        let self_da = DA::from_coeff(*self);
        let self_cd = CD::from_da(&self_da);
        self_cd.rosy_add(other)
    }
}

// LO + LO (Logical OR)
impl RosyAdd<&LO> for &LO {
    type Output = LO;
    fn rosy_add(self, other: &LO) -> Result<Self::Output> {
        Ok(*self || *other)
    }
}

// CM + RE
impl RosyAdd<&DA> for &CM {
    type Output = CD;
    fn rosy_add(self, other: &DA) -> Result<Self::Output> {
        // Create CD from the complex number
        let cm_cd = CD::complex_constant(*self);
        // Create CD from the DA (which becomes the real part)
        let da_cd = CD::from_da(other);
        // Add them
        Ok((&cm_cd + &da_cd)?)
    }
}

// CM + CD
impl RosyAdd<&CD> for &CM {
    type Output = CD;
    fn rosy_add(self, other: &CD) -> Result<Self::Output> {
        other + *self
    }
}

// DA + CM
impl RosyAdd<&CM> for &DA {
    type Output = CD;
    fn rosy_add(self, other: &CM) -> Result<Self::Output> {
        
        // Create CD from the complex number
        let cm_cd = CD::complex_constant(*other);

        // Create CD from the DA (which becomes the real part)

        let da_cd = CD::from_da(self);
        // Add them
        &da_cd + &cm_cd
    }
}

// DA + CD
impl RosyAdd<&CD> for &DA {
    type Output = CD;
    fn rosy_add(self, other: &CD) -> Result<Self::Output> {
        // Create CD from DA (real part only, imaginary is zero)
        let self_cd = CD::from_da(self);
        &self_cd + other
    }
}

// CD + RE
impl RosyAdd<&RE> for &CD {
    type Output = CD;
    fn rosy_add(self, other: &RE) -> Result<Self::Output> {
        self + Complex64::new(*other, 0.0)
    }
}

// CD + CM
impl RosyAdd<&CM> for &CD {
    type Output = CD;
    fn rosy_add(self, other: &CM) -> Result<Self::Output> {
        self + *other
    }
}

// CD + DA
impl RosyAdd<&DA> for &CD {
    type Output = CD;
    fn rosy_add(self, other: &DA) -> Result<Self::Output> {
        // Create CD from DA (real part only, imaginary is zero)
        let other_cd = CD::from_da(other);
        self + &other_cd
    }
}

// CD + CD
impl RosyAdd<&CD> for &CD {
    type Output = CD;
    fn rosy_add(self, other: &CD) -> Result<Self::Output> {
        self + other
    }
}
