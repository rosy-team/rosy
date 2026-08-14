use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, CM, VE, DA, CD};

/// Type registry for CONS intrinsic function.
///
/// According to COSY INFINITY manual, CONS supports:
/// - RE -> RE (identity)
/// - CM -> CM (identity)
/// - VE -> RE (max abs value)
/// - DA -> RE (constant part)
/// - CD -> CM (constant part of complex DA)
pub const CONS_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("CM", "CM", "CM(1.5&2.5)"),
    IntrinsicTypeRule::new("VE", "RE", "1.5&2.5&3.5"),
    IntrinsicTypeRule::new("DA", "RE", "DA(1)"),
    IntrinsicTypeRule::new("CD", "CM", "CD(1)"),
];

/// Get the return type of CONS for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::RE()),
            (RosyType::CM(), RosyType::CM()),
            (RosyType::VE(), RosyType::RE()),
            (RosyType::DA(), RosyType::RE()),
            (RosyType::CD(), RosyType::CM()),
        ];
        for (input_type, result_type) in all {
            m.insert(input_type, result_type);
        }
        m
    };

    registry.get(input).copied()
}

/// Trait for extracting the constant part of Rosy data types.
pub trait RosyCONS {
    type Output;
    fn rosy_cons(&self) -> anyhow::Result<Self::Output>;
}

/// CONS for real numbers - identity
impl RosyCONS for RE {
    type Output = RE;
    fn rosy_cons(&self) -> anyhow::Result<Self::Output> {
        Ok(*self)
    }
}

/// CONS for complex numbers - identity
impl RosyCONS for CM {
    type Output = CM;
    fn rosy_cons(&self) -> anyhow::Result<Self::Output> {
        Ok(*self)
    }
}

/// CONS for vectors - max abs value
impl RosyCONS for VE {
    type Output = RE;
    fn rosy_cons(&self) -> anyhow::Result<Self::Output> {
        if self.is_empty() {
            anyhow::bail!("CONS called on empty vector");
        }
        Ok(self.iter().map(|x| x.abs()).fold(0.0f64, f64::max))
    }
}

/// CONS for DA - constant part
impl RosyCONS for DA {
    type Output = RE;
    fn rosy_cons(&self) -> anyhow::Result<Self::Output> {
        Ok(self.constant_part())
    }
}

/// CONS for CD - constant part (complex)
impl RosyCONS for CD {
    type Output = CM;
    fn rosy_cons(&self) -> anyhow::Result<Self::Output> {
        Ok(self.constant_part())
    }
}
