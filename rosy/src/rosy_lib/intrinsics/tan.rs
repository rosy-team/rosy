use std::collections::HashMap;

use crate::rosy_lib::{CD, DA, RE, VE};
use crate::rosy_lib::{IntrinsicTypeRule, RosyType};

/// Type registry for TAN intrinsic function.
///
/// According to COSY INFINITY manual, TAN supports:
/// - RE -> RE
/// - VE -> VE (elementwise)
/// - DA -> DA (Taylor composition)
///
/// Note: CM is NOT supported for TAN in COSY.
pub const TAN_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("VE", "VE", "1.5&2.5&3.5"),
    IntrinsicTypeRule::new("DA", "DA", "DA(1)"),
];

/// Get the return type of TAN for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::RE()),
            (RosyType::VE(), RosyType::VE()),
            (RosyType::DA(), RosyType::DA()),
        ];
        for (input_type, result_type) in all {
            m.insert(input_type, result_type);
        }
        m
    };

    registry.get(input).copied()
}

/// Trait for computing the tangent of Rosy data types.
pub trait RosyTAN {
    type Output;
    fn rosy_tan(&self) -> anyhow::Result<Self::Output>;
}

/// TAN for real numbers
impl RosyTAN for RE {
    type Output = RE;
    fn rosy_tan(&self) -> anyhow::Result<Self::Output> {
        Ok(self.tan())
    }
}

/// TAN for vectors (elementwise)
impl RosyTAN for VE {
    type Output = VE;
    fn rosy_tan(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.tan()).collect())
    }
}

/// TAN for DA (Taylor composition)
/// Uses: tan(f) = sin(f) / cos(f)
impl RosyTAN for DA {
    type Output = DA;
    fn rosy_tan(&self) -> anyhow::Result<Self::Output> {
        use crate::rosy_lib::intrinsics::cos::RosyCOS;
        use crate::rosy_lib::intrinsics::sin::RosySIN;

        let sin_f = self.rosy_sin()?;
        let cos_f = self.rosy_cos()?;

        (&sin_f / &cos_f).map_err(|e| e)
    }
}
