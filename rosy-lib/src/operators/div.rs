//! Division operator for Rosy types.

use anyhow::Result;
use crate::RosyType;
use crate::{RE, CM, VE, DA, CD};

pub fn get_return_type(lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    crate::operators::arith_return(lhs, rhs, false)
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
