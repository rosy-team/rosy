use std::collections::HashMap;

use crate::rosy_lib::{CD, CM, DA, LO, RE, ST, VE};
use crate::rosy_lib::{IntrinsicTypeRule, RosyType};

/// Type registry for LENGTH intrinsic function.
///
/// According to COSY INFINITY manual, LENGTH returns RE for all types:
/// - RE -> RE
/// - ST -> RE  
/// - LO -> RE
/// - CM -> RE
/// - VE -> RE
/// - DA -> RE
/// - CD -> RE
/// - GR -> RE (not implemented yet)
pub const LENGTH_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("ST", "RE", "\"Hello\""),
    IntrinsicTypeRule::new("LO", "RE", "TRUE"),
    IntrinsicTypeRule::new("CM", "RE", "1.5&2.5"),
    IntrinsicTypeRule::new("VE", "RE", "1.5&2.5&3.5"),
    IntrinsicTypeRule::new("DA", "RE", "1.5+DA(1)"),
    IntrinsicTypeRule::new("CD", "RE", "CM(1.5&2.5)+CD(1)"),
];

/// Get the return type of LENGTH for a given input type.
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

/// Trait for getting the LENGTH (memory size in 8-byte blocks) of Rosy data types.
pub trait RosyLENGTH {
    fn rosy_length(&self) -> RE;
}

/// LENGTH for real numbers - always 1 block
impl RosyLENGTH for RE {
    fn rosy_length(&self) -> RE {
        1.0
    }
}

/// LENGTH for strings - returns the number of characters
impl RosyLENGTH for ST {
    fn rosy_length(&self) -> RE {
        self.len() as f64
    }
}

/// LENGTH for booleans - always 1 block
impl RosyLENGTH for LO {
    fn rosy_length(&self) -> RE {
        1.0
    }
}

/// LENGTH for complex numbers - always 2 blocks (real + imaginary)
impl RosyLENGTH for CM {
    fn rosy_length(&self) -> RE {
        2.0
    }
}

/// LENGTH for vectors - number of elements
impl RosyLENGTH for VE {
    fn rosy_length(&self) -> RE {
        self.len() as f64
    }
}

/// LENGTH for DA - depends on DA storage requirements
impl RosyLENGTH for DA {
    fn rosy_length(&self) -> RE {
        // DA stores f64 coefficients, each coefficient is 1 block (8 bytes)
        // Return the number of terms (coefficients) stored
        self.num_terms() as f64
    }
}

/// LENGTH for CD - depends on CD storage requirements
impl RosyLENGTH for CD {
    fn rosy_length(&self) -> RE {
        // CD stores complex coefficients (real + imaginary parts)
        // Each complex coefficient = 2 f64 values = 2 blocks (16 bytes)
        self.num_terms() as f64 * 2.0
    }
}
