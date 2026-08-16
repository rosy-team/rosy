use crate::RosyType;
use crate::{RE, ST, LO, CM, VE, DA, CD};

/// Get the return type of VARMEM for a given input type.
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

/// Trait for getting the memory address of Rosy data types.
///
/// Since Rosy transpiles to Rust (not Fortran), true COSY memory addresses
/// are meaningless. VARMEM returns the actual Rust pointer address cast to f64,
/// giving meaningful unique values for debugging.
pub trait RosyVARMEM {
    fn rosy_varmem(&self) -> RE;
}

/// VARMEM for real numbers - returns pointer address as f64
impl RosyVARMEM for RE {
    fn rosy_varmem(&self) -> RE {
        self as *const Self as usize as f64
    }
}

/// VARMEM for strings - returns pointer address as f64
impl RosyVARMEM for ST {
    fn rosy_varmem(&self) -> RE {
        self as *const Self as usize as f64
    }
}

/// VARMEM for booleans - returns pointer address as f64
impl RosyVARMEM for LO {
    fn rosy_varmem(&self) -> RE {
        self as *const Self as usize as f64
    }
}

/// VARMEM for complex numbers - returns pointer address as f64
impl RosyVARMEM for CM {
    fn rosy_varmem(&self) -> RE {
        self as *const Self as usize as f64
    }
}

/// VARMEM for vectors - returns pointer address as f64
impl RosyVARMEM for VE {
    fn rosy_varmem(&self) -> RE {
        self as *const Self as usize as f64
    }
}

/// VARMEM for DA - returns pointer address as f64
impl RosyVARMEM for DA {
    fn rosy_varmem(&self) -> RE {
        self as *const Self as usize as f64
    }
}

/// VARMEM for CD - returns pointer address as f64
impl RosyVARMEM for CD {
    fn rosy_varmem(&self) -> RE {
        self as *const Self as usize as f64
    }
}
