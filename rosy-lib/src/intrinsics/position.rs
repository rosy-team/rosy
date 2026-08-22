//! POSITION intrinsic function — find substring position.
//!
//! Returns the 1-based index of the first occurrence of `needle` in `haystack`,
//! or 0 if not found. Matches COSY INFINITY behavior.

use crate::{RosyType, RosyBaseType, RE, ST};

/// Get the return type of POSITION for given input types.
pub fn get_return_type(haystack: &RosyType, needle: &RosyType) -> Option<RosyType> {
    match (&haystack.base_type, &needle.base_type) {
        (RosyBaseType::ST, RosyBaseType::ST) => Some(RosyType::new(RosyBaseType::RE, 0)),
        _ => None,
    }
}

/// Trait for the POSITION intrinsic.
pub trait RosyPOSITION {
    fn rosy_position(&self, needle: &ST) -> RE;
}

impl RosyPOSITION for ST {
    fn rosy_position(&self, needle: &ST) -> RE {
        self.find(needle.as_str())
            .map(|i| (i + 1) as f64)
            .unwrap_or(0.0)
    }
}

impl RosyPOSITION for crate::RosyValue {
    fn rosy_position(&self, needle: &ST) -> RE {
        self.clone().expect_st().map(|s| s.rosy_position(needle)).unwrap_or(0.0)
    }
}
