use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, VE};

/// Type registry for INT intrinsic function.
///
/// INT truncates toward zero. Supports:
/// - RE -> RE (f64::trunc)
/// - VE -> VE (elementwise trunc)
pub const INT_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.7"),
    IntrinsicTypeRule::new("VE", "VE", "1.7&2.3&3.9"),
];

/// Get the return type of INT for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        m.insert(RosyType::RE(), RosyType::RE());
        m.insert(RosyType::VE(), RosyType::VE());
        m
    };
    registry.get(input).copied()
}

/// Trait for truncating Rosy data types toward zero.
pub trait RosyINT {
    type Output;
    fn rosy_int(&self) -> anyhow::Result<Self::Output>;
}

/// INT for real numbers - truncate toward zero
impl RosyINT for RE {
    type Output = RE;
    fn rosy_int(&self) -> anyhow::Result<RE> {
        Ok(self.trunc())
    }
}

/// INT for vectors - elementwise truncation
impl RosyINT for VE {
    type Output = VE;
    fn rosy_int(&self) -> anyhow::Result<VE> {
        Ok(self.iter().map(|x| x.trunc()).collect())
    }
}
