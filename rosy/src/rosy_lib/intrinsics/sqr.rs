use std::collections::HashMap;

use crate::rosy_lib::{CD, CM, DA, RE, VE};
use crate::rosy_lib::{IntrinsicTypeRule, RosyType};

/// Type registry for SQR intrinsic function.
///
/// SQR computes the square (x²). Supports:
/// - RE -> RE
/// - CM -> CM
/// - VE -> VE (elementwise)
/// - DA -> DA
/// - CD -> CD
pub const SQR_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("CM", "CM", "CM(1.5&2.5)"),
    IntrinsicTypeRule::new("VE", "VE", "1.5&2.5&3.5"),
    IntrinsicTypeRule::new("DA", "DA", "DA(1)"),
    IntrinsicTypeRule::new("CD", "CD", "CD(1)"),
];

/// Get the return type of SQR for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::RE()),
            (RosyType::CM(), RosyType::CM()),
            (RosyType::VE(), RosyType::VE()),
            (RosyType::DA(), RosyType::DA()),
            (RosyType::CD(), RosyType::CD()),
        ];
        for (input_type, result_type) in all {
            m.insert(input_type, result_type);
        }
        m
    };

    registry.get(input).copied()
}

/// Trait for computing the square of Rosy data types.
pub trait RosySQR {
    type Output;
    fn rosy_sqr(&self) -> anyhow::Result<Self::Output>;
}

/// SQR for real numbers
impl RosySQR for RE {
    type Output = RE;
    fn rosy_sqr(&self) -> anyhow::Result<Self::Output> {
        Ok(self * self)
    }
}

/// SQR for complex numbers
impl RosySQR for CM {
    type Output = CM;
    fn rosy_sqr(&self) -> anyhow::Result<Self::Output> {
        Ok(self * self)
    }
}

/// SQR for vectors (elementwise)
impl RosySQR for VE {
    type Output = VE;
    fn rosy_sqr(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x * x).collect())
    }
}

/// SQR for DA (Taylor multiplication)
impl RosySQR for DA {
    type Output = DA;
    fn rosy_sqr(&self) -> anyhow::Result<Self::Output> {
        (self * self).map_err(|e| e)
    }
}

/// SQR for CD (complex Taylor multiplication)
impl RosySQR for CD {
    type Output = CD;
    fn rosy_sqr(&self) -> anyhow::Result<Self::Output> {
        (self * self).map_err(|e| e)
    }
}
