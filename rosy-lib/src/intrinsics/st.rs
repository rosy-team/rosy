use std::collections::HashMap;

use crate::RosyType;
use crate::{RE, CM, VE, LO, ST, DA, CD};
use crate::core::display::RosyDisplay;

pub fn get_return_type ( lhs: &RosyType ) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec!(
            (RosyType::RE(), RosyType::ST()),
            (RosyType::ST(), RosyType::ST()),
            (RosyType::LO(), RosyType::ST()),
            (RosyType::CM(), RosyType::ST()),
            (RosyType::VE(), RosyType::ST()),
            (RosyType::DA(), RosyType::ST()),
            (RosyType::CD(), RosyType::ST()),
        );
        for (left, result) in all {
            m.insert(left, result);
        }
        m
    };

    registry.get(&*lhs).copied()
}           

/// Trait for converting Rosy data types to strings
pub trait RosyST {
    fn rosy_to_string(self) -> String;
}

/// Convert real numbers to strings
impl RosyST for &RE {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

/// Convert strings to strings (identity)
impl RosyST for &ST {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

/// Convert booleans to strings
impl RosyST for &LO {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

/// Convert vectors to strings
impl RosyST for &VE {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

/// Convert complex numbers to strings
impl RosyST for &CM {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

/// Convert Differential Algebra (DA) to strings
impl RosyST for &DA {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

/// Convert Complex Differential Algebra (CD) to strings
impl RosyST for &CD {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}