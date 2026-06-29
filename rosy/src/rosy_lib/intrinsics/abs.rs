use std::collections::HashMap;

use crate::rosy_lib::{CD, CM, DA, RE, VE};
use crate::rosy_lib::{IntrinsicTypeRule, RosyType};

/// Type registry for ABS intrinsic function.
///
/// ABS computes the absolute value. Supports:
/// - RE -> RE (f64::abs)
/// - CM -> RE (Complex modulus / norm)
/// - VE -> RE (sum of absolute values of elements)
/// - DA -> RE (max absolute value among coefficients)
pub const ABS_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("CM", "RE", "CM(3.0&4.0)"),
    IntrinsicTypeRule::new("VE", "RE", "1.5&2.5&3.5"),
    IntrinsicTypeRule::new("DA", "RE", "DA(1)"),
    IntrinsicTypeRule::new("CD", "RE", "CD(1)"),
];

/// Get the return type of ABS for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        m.insert(RosyType::RE(), RosyType::RE());
        m.insert(RosyType::CM(), RosyType::RE());
        m.insert(RosyType::VE(), RosyType::RE());
        m.insert(RosyType::DA(), RosyType::RE());
        m.insert(RosyType::CD(), RosyType::RE());
        m
    };
    registry.get(input).copied()
}

/// Trait for computing the absolute value of Rosy data types.
pub trait RosyABS {
    type Output;
    fn rosy_abs(&self) -> anyhow::Result<Self::Output>;
}

/// ABS for real numbers
impl RosyABS for RE {
    type Output = RE;
    fn rosy_abs(&self) -> anyhow::Result<RE> {
        Ok(self.abs())
    }
}

/// ABS for complex numbers - returns the modulus (norm)
impl RosyABS for CM {
    type Output = RE;
    fn rosy_abs(&self) -> anyhow::Result<RE> {
        Ok(self.norm())
    }
}

/// ABS for vectors - returns sum of absolute values
impl RosyABS for VE {
    type Output = RE;
    fn rosy_abs(&self) -> anyhow::Result<RE> {
        Ok(self.iter().map(|x| x.abs()).sum())
    }
}

/// ABS for DA - returns max absolute value among all coefficients
impl RosyABS for DA {
    type Output = RE;
    fn rosy_abs(&self) -> anyhow::Result<RE> {
        let max_coeff = self
            .coeffs_iter()
            .into_iter()
            .map(|(_, c)| c.abs())
            .fold(0.0_f64, f64::max);
        Ok(max_coeff)
    }
}

/// ABS for CD - returns max absolute value (norm) among all complex coefficients
impl RosyABS for CD {
    type Output = RE;
    fn rosy_abs(&self) -> anyhow::Result<RE> {
        use crate::rosy_lib::taylor::DACoefficient;
        Ok(self
            .coeffs_iter()
            .into_iter()
            .map(|(_, c)| c.abs())
            .fold(0.0_f64, f64::max))
    }
}
