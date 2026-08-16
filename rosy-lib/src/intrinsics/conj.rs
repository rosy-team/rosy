use crate::RosyType;
use crate::{RE, CM, CD};

/// Get the return type of CONJ for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::CM()),
        t if *t == RosyType::CD() => Some(RosyType::CD()),
        _ => None,
    }
}

/// Trait for computing complex conjugate of Rosy data types.
pub trait RosyCONJ {
    type Output;
    fn rosy_conj(&self) -> anyhow::Result<Self::Output>;
}

/// CONJ for real numbers: identity (real numbers are self-conjugate)
impl RosyCONJ for RE {
    type Output = RE;
    fn rosy_conj(&self) -> anyhow::Result<Self::Output> {
        Ok(*self)
    }
}

/// CONJ for complex numbers: complex conjugate
impl RosyCONJ for CM {
    type Output = CM;
    fn rosy_conj(&self) -> anyhow::Result<Self::Output> {
        Ok(self.conj())
    }
}

/// CONJ for CD: conjugate each complex coefficient in the Taylor series
impl RosyCONJ for CD {
    type Output = CD;
    fn rosy_conj(&self) -> anyhow::Result<Self::Output> {
        use crate::taylor::DACoefficient;
        use num_complex::Complex64;
        let mut result = CD::from_coeff(Complex64::zero());
        for (monomial, coeff) in self.coeffs_iter() {
            result.set_coeff(monomial.clone(), coeff.conj());
        }
        Ok(result)
    }
}

