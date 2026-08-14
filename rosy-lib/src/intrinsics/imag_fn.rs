use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, CM, DA, CD};

/// Type registry for IMAG intrinsic function.
///
/// According to COSY INFINITY manual, IMAG supports:
/// - RE -> RE (returns 0.0)
/// - CM -> RE (imaginary part of complex)
/// - DA -> DA (returns zero DA)
pub const IMAG_FN_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("CM", "RE", "CM(1.5&2.5)"),
    IntrinsicTypeRule::new("DA", "DA", "DA(1)"),
    IntrinsicTypeRule::new("CD", "DA", "CD(1)"),
];

/// Get the return type of IMAG for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::RE()),
            (RosyType::CM(), RosyType::RE()),
            (RosyType::DA(), RosyType::DA()),
            (RosyType::CD(), RosyType::DA()),
        ];
        for (input_type, result_type) in all {
            m.insert(input_type, result_type);
        }
        m
    };

    registry.get(input).copied()
}

/// Trait for computing the imaginary part of Rosy data types.
pub trait RosyIMAG {
    type Output;
    fn rosy_imag(&self) -> anyhow::Result<Self::Output>;
}

/// IMAG for real numbers - returns 0.0
impl RosyIMAG for RE {
    type Output = RE;
    fn rosy_imag(&self) -> anyhow::Result<Self::Output> {
        Ok(0.0)
    }
}

/// IMAG for complex numbers - imaginary part
impl RosyIMAG for CM {
    type Output = RE;
    fn rosy_imag(&self) -> anyhow::Result<Self::Output> {
        Ok(self.im)
    }
}

/// IMAG for DA - returns zero DA
impl RosyIMAG for DA {
    type Output = DA;
    fn rosy_imag(&self) -> anyhow::Result<Self::Output> {
        Ok(DA::from_coeff(0.0))
    }
}

/// IMAG for CD - extract imaginary part of each complex coefficient
impl RosyIMAG for CD {
    type Output = DA;
    fn rosy_imag(&self) -> anyhow::Result<Self::Output> {
        Ok(self.imag_part())
    }
}

