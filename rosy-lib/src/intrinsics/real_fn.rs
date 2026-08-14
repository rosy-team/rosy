use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, CM, DA, CD};

/// Type registry for REAL intrinsic function.
///
/// According to COSY INFINITY manual, REAL supports:
/// - RE -> RE (identity)
/// - CM -> RE (real part of complex)
/// - DA -> DA (identity)
pub const REAL_FN_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("CM", "RE", "CM(1.5&2.5)"),
    IntrinsicTypeRule::new("DA", "DA", "DA(1)"),
    IntrinsicTypeRule::new("CD", "DA", "CD(1)"),
];

/// Get the return type of REAL for a given input type.
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

/// Trait for computing the real part of Rosy data types.
pub trait RosyREAL {
    type Output;
    fn rosy_real(&self) -> anyhow::Result<Self::Output>;
}

/// REAL for real numbers - identity
impl RosyREAL for RE {
    type Output = RE;
    fn rosy_real(&self) -> anyhow::Result<Self::Output> {
        Ok(*self)
    }
}

/// REAL for complex numbers - real part
impl RosyREAL for CM {
    type Output = RE;
    fn rosy_real(&self) -> anyhow::Result<Self::Output> {
        Ok(self.re)
    }
}

/// REAL for DA - identity
impl RosyREAL for DA {
    type Output = DA;
    fn rosy_real(&self) -> anyhow::Result<Self::Output> {
        Ok(self.clone())
    }
}

/// REAL for CD - extract real part of each complex coefficient
impl RosyREAL for CD {
    type Output = DA;
    fn rosy_real(&self) -> anyhow::Result<Self::Output> {
        Ok(self.real_part())
    }
}

