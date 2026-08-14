use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, CM, CD};

/// Type registry for CONJ intrinsic function (complex conjugate).
///
/// According to COSY INFINITY manual, CONJ supports:
/// - RE -> RE (identity, real numbers are their own conjugate)
/// - CM -> CM (complex conjugate)
pub const CONJ_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("CM", "CM", "CM(1.5&2.5)"),
    IntrinsicTypeRule::new("CD", "CD", "CD(1)"),
];

/// Get the return type of CONJ for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::RE()),
            (RosyType::CM(), RosyType::CM()),
            (RosyType::CD(), RosyType::CD()),
        ];
        for (input_type, result_type) in all {
            m.insert(input_type, result_type);
        }
        m
    };

    registry.get(input).copied()
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
        use crate::taylor::{DACoefficient, Monomial};
        use num_complex::Complex64;
        let mut result = CD::from_coeff(Complex64::zero());
        for (monomial, coeff) in self.coeffs_iter() {
            result.set_coeff(monomial.clone(), coeff.conj());
        }
        Ok(result)
    }
}

