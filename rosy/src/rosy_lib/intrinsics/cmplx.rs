use std::collections::HashMap;

use crate::rosy_lib::{CD, CM, DA, RE};
use crate::rosy_lib::{IntrinsicTypeRule, RosyType};

/// Type registry for CMPLX intrinsic function (convert to complex).
///
/// According to COSY INFINITY manual, CMPLX supports:
/// - RE -> CM
/// - CM -> CM (identity)
pub const CMPLX_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "CM", "1.5"),
    IntrinsicTypeRule::new("CM", "CM", "CM(1.5&2.5)"),
    IntrinsicTypeRule::new("DA", "CD", "DA(1)"),
    IntrinsicTypeRule::new("CD", "CD", "CD(1)"),
];

/// Get the return type of CMPLX for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::CM()),
            (RosyType::CM(), RosyType::CM()),
            (RosyType::DA(), RosyType::CD()),
            (RosyType::CD(), RosyType::CD()),
        ];
        for (input_type, result_type) in all {
            m.insert(input_type, result_type);
        }
        m
    };

    registry.get(input).copied()
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
