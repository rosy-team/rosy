use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, VE};

/// Type registry for NINT intrinsic function.
///
/// NINT rounds to the nearest integer. Supports:
/// - RE -> RE (f64::round)
/// - VE -> VE (elementwise round)
pub const NINT_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.7"),
    IntrinsicTypeRule::new("VE", "VE", "1.7&2.3&3.9"),
];

/// Get the return type of NINT for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        m.insert(RosyType::RE(), RosyType::RE());
        m.insert(RosyType::VE(), RosyType::VE());
        m
    };
    registry.get(input).copied()
}

/// Trait for rounding Rosy data types to the nearest integer.
pub trait RosyNINT {
    type Output;
    fn rosy_nint(&self) -> anyhow::Result<Self::Output>;
}

/// NINT for real numbers - round to nearest integer
impl RosyNINT for RE {
    type Output = RE;
    fn rosy_nint(&self) -> anyhow::Result<RE> {
        Ok(self.round())
    }
}

/// NINT for vectors - elementwise rounding
impl RosyNINT for VE {
    type Output = VE;
    fn rosy_nint(&self) -> anyhow::Result<VE> {
        Ok(self.iter().map(|x| x.round()).collect())
    }
}
