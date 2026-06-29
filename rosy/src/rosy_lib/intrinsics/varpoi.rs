use std::collections::HashMap;

use crate::rosy_lib::{CD, CM, DA, LO, RE, ST, VE};
use crate::rosy_lib::{IntrinsicTypeRule, RosyType};

/// Type registry for VARPOI intrinsic function.
///
/// According to COSY INFINITY manual, VARPOI returns RE for all types:
/// - RE -> RE
/// - ST -> RE
/// - LO -> RE
/// - CM -> RE
/// - VE -> RE
/// - DA -> RE
/// - CD -> RE
///
/// In Rosy, VARPOI returns the Rust pointer address cast to f64,
/// identical in behavior to VARMEM (Rust has no Fortran-style pointer/memory distinction).
pub const VARPOI_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("ST", "RE", "\"Hello\""),
    IntrinsicTypeRule::new("LO", "RE", "TRUE"),
    IntrinsicTypeRule::new("CM", "RE", "1.5&2.5"),
    IntrinsicTypeRule::new("VE", "RE", "1.5&2.5&3.5"),
    IntrinsicTypeRule::new("DA", "RE", "1.5+DA(1)"),
    IntrinsicTypeRule::new("CD", "RE", "CM(1.5&2.5)+CD(1)"),
];

/// Get the return type of VARPOI for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::RE()),
            (RosyType::ST(), RosyType::RE()),
            (RosyType::LO(), RosyType::RE()),
            (RosyType::CM(), RosyType::RE()),
            (RosyType::VE(), RosyType::RE()),
            (RosyType::DA(), RosyType::RE()),
            (RosyType::CD(), RosyType::RE()),
        ];
        for (input_type, result_type) in all {
            m.insert(input_type, result_type);
        }
        m
    };

    registry.get(input).copied()
}

/// Trait for getting the pointer address of Rosy data types.
pub trait RosyVARPOI {
    fn rosy_varpoi(&self) -> RE;
}

/// VARPOI for real numbers - returns pointer address as f64
impl RosyVARPOI for RE {
    fn rosy_varpoi(&self) -> RE {
        self as *const Self as usize as f64
    }
}

/// VARPOI for strings - returns pointer address as f64
impl RosyVARPOI for ST {
    fn rosy_varpoi(&self) -> RE {
        self as *const Self as usize as f64
    }
}

/// VARPOI for booleans - returns pointer address as f64
impl RosyVARPOI for LO {
    fn rosy_varpoi(&self) -> RE {
        self as *const Self as usize as f64
    }
}

/// VARPOI for complex numbers - returns pointer address as f64
impl RosyVARPOI for CM {
    fn rosy_varpoi(&self) -> RE {
        self as *const Self as usize as f64
    }
}

/// VARPOI for vectors - returns pointer address as f64
impl RosyVARPOI for VE {
    fn rosy_varpoi(&self) -> RE {
        self as *const Self as usize as f64
    }
}

/// VARPOI for DA - returns pointer address as f64
impl RosyVARPOI for DA {
    fn rosy_varpoi(&self) -> RE {
        self as *const Self as usize as f64
    }
}

/// VARPOI for CD - returns pointer address as f64
impl RosyVARPOI for CD {
    fn rosy_varpoi(&self) -> RE {
        self as *const Self as usize as f64
    }
}
