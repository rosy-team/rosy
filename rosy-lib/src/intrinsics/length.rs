use crate::RosyType;
use crate::{RE, ST, LO, CM, VE, DA, CD};

/// Get the return type of LENGTH for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::ST() => Some(RosyType::RE()),
        t if *t == RosyType::LO() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::RE()),
        t if *t == RosyType::DA() => Some(RosyType::RE()),
        t if *t == RosyType::CD() => Some(RosyType::RE()),
        _ => None,
    }
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