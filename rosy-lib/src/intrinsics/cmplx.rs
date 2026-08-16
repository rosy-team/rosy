use crate::RosyType;
use crate::{RE, CM, DA, CD};

/// Get the return type of CMPLX for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::CM()),
        t if *t == RosyType::CM() => Some(RosyType::CM()),
        t if *t == RosyType::DA() => Some(RosyType::CD()),
        t if *t == RosyType::CD() => Some(RosyType::CD()),
        _ => None,
    }
}

/// Trait for converting Rosy data types to complex.
pub trait RosyCMPLX {
    type Output;
    fn rosy_cmplx(&self) -> anyhow::Result<Self::Output>;
}

/// CMPLX for real numbers: RE -> CM
impl RosyCMPLX for RE {
    type Output = CM;
    fn rosy_cmplx(&self) -> anyhow::Result<Self::Output> {
        Ok(num_complex::Complex64::new(*self, 0.0))
    }
}

/// CMPLX for complex numbers: CM -> CM (identity)
impl RosyCMPLX for CM {
    type Output = CM;
    fn rosy_cmplx(&self) -> anyhow::Result<Self::Output> {
        Ok(*self)
    }
}

/// CMPLX for DA: DA -> CD (promote real Taylor series to complex Taylor series)
impl RosyCMPLX for DA {
    type Output = CD;
    fn rosy_cmplx(&self) -> anyhow::Result<Self::Output> {
        Ok(CD::from_da(self))
    }
}

/// CMPLX for CD: CD -> CD (identity)
impl RosyCMPLX for CD {
    type Output = CD;
    fn rosy_cmplx(&self) -> anyhow::Result<Self::Output> {
        Ok(self.clone())
    }
}

