use crate::RosyType;
use crate::{RE, LO};

pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::LO()),
        t if *t == RosyType::LO() => Some(RosyType::LO()),
        _ => None,
    }
}           

/// Trait for converting Rosy data types to strings
pub trait RosyLO {
    fn rosy_to_logical(self) -> LO;
}

/// Convert real numbers to logical
impl RosyLO for &RE {
    fn rosy_to_logical(self) -> LO {
        if *self != 0.0 {
            true
        } else {
            false
        }
    }
}

/// Convert strings to strings (identity)
impl RosyLO for &LO {
    fn rosy_to_logical(self) -> LO {
        *self
    }
}