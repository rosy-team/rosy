use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, CM, VE, ST, DA};

/// Type registry for RE() conversion function.
///
/// According to COSY INFINITY manual, RE() supports:
/// - RE -> RE (identity)
/// - ST -> RE (parse string as f64)
/// - CM -> RE (extracts real part)
/// - VE -> RE (average)
/// - DA -> RE (constant part)
pub const RE_CONVERT_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("ST", "RE", "\"3.14\""),
    IntrinsicTypeRule::new("CM", "RE", "CM(1.5&2.5)"),
    IntrinsicTypeRule::new("VE", "RE", "1.5&2.5&3.5"),
    IntrinsicTypeRule::new("DA", "RE", "DA(1)"),
];

/// Get the return type of RE() for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::RE()),
            (RosyType::ST(), RosyType::RE()),
            (RosyType::CM(), RosyType::RE()),
            (RosyType::VE(), RosyType::RE()),
            (RosyType::DA(), RosyType::RE()),
        ];
        for (input_type, result_type) in all {
            m.insert(input_type, result_type);
        }
        m
    };

    registry.get(input).copied()
}

/// Trait for converting Rosy data types to real (RE).
pub trait RosyREConvert {
    fn rosy_re_convert(&self) -> anyhow::Result<RE>;
}

/// RE -> RE identity
impl RosyREConvert for RE {
    fn rosy_re_convert(&self) -> anyhow::Result<RE> {
        Ok(*self)
    }
}

/// ST -> RE (parse string as f64)
impl RosyREConvert for ST {
    fn rosy_re_convert(&self) -> anyhow::Result<RE> {
        self.trim().parse::<f64>()
            .map_err(|e| anyhow::anyhow!("Failed to convert ST to RE: {}", e))
    }
}

/// CM -> RE (real part)
impl RosyREConvert for CM {
    fn rosy_re_convert(&self) -> anyhow::Result<RE> {
        Ok(self.re)
    }
}

/// VE -> RE (average)
impl RosyREConvert for VE {
    fn rosy_re_convert(&self) -> anyhow::Result<RE> {
        if self.is_empty() {
            anyhow::bail!("RE() called on empty vector");
        }
        Ok(self.iter().sum::<f64>() / self.len() as f64)
    }
}

/// DA -> RE (constant part)
impl RosyREConvert for DA {
    fn rosy_re_convert(&self) -> anyhow::Result<RE> {
        Ok(self.constant_part())
    }
}
