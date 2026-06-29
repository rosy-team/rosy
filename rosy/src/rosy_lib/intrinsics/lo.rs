use std::collections::HashMap;

use crate::rosy_lib::RosyType;
use crate::rosy_lib::core::display::RosyDisplay;
use crate::rosy_lib::{CD, CM, DA, LO, RE, ST, VE};

pub fn get_return_type(lhs: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::LO()),
            (RosyType::LO(), RosyType::LO()),
        ];
        for (left, result) in all {
            m.insert(left, result);
        }
        m
    };

    registry.get(&*lhs).copied()
}

/// Trait for converting Rosy data types to strings
pub trait RosyLO {
    fn rosy_to_logical(self) -> LO;
}

/// Convert real numbers to logical
impl RosyLO for &RE {
    fn rosy_to_logical(self) -> LO {
        if *self != 0.0 { true } else { false }
    }
}

/// Convert strings to strings (identity)
impl RosyLO for &LO {
    fn rosy_to_logical(self) -> LO {
        *self
    }
}
