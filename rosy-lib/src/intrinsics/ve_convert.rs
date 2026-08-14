use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{CM, RE, VE};

/// Type registry for VE() conversion function.
///
/// VE() supports:
/// - RE -> VE (single-element vector)
/// - CM -> VE (two-vector of real, imaginary parts)
/// - VE -> VE (identity)
pub const VE_CONVERT_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "VE", "1.5"),
    IntrinsicTypeRule::new("CM", "VE", "CM(1.5&2.5)"),
    IntrinsicTypeRule::new("VE", "VE", "1.5&2.5&3.5"),
];

/// Get the return type of VE() for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::VE()),
            (RosyType::CM(), RosyType::VE()),
            (RosyType::VE(), RosyType::VE()),
        ];
        for (input_type, result_type) in all {
            m.insert(input_type, result_type);
        }
        m
    };

    registry.get(input).copied()
}

/// Trait for converting Rosy data types to vectors (VE).
pub trait RosyVEConvert {
    fn rosy_ve_convert(&self) -> anyhow::Result<VE>;
}

/// RE -> VE (single-element vector)
impl RosyVEConvert for RE {
    fn rosy_ve_convert(&self) -> anyhow::Result<VE> {
        Ok(vec![*self])
    }
}

/// CM -> VE (two-vector of real, imaginary parts)
impl RosyVEConvert for CM {
    fn rosy_ve_convert(&self) -> anyhow::Result<VE> {
        Ok(vec![self.re, self.im])
    }
}

/// VE -> VE identity
impl RosyVEConvert for VE {
    fn rosy_ve_convert(&self) -> anyhow::Result<VE> {
        Ok(self.clone())
    }
}
