use crate::RosyType;
use crate::{RE, ST, LO, CM, VE, DA, CD};

/// Get the return type of VARPOI for a given input type.
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
