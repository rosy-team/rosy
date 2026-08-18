use crate::{RE, VE};
use crate::RosyType;

/// Get the return type of VMIN for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    if *input == RosyType::VE() || *input == RosyType::RE() {
        Some(RosyType::RE())
    } else {
        None
    }
}

/// Trait for computing the minimum element of Rosy vector types.
pub trait RosyVMIN {
    fn rosy_vmin(&self) -> anyhow::Result<RE>;
}

/// VMIN for vectors - returns the minimum element
impl RosyVMIN for RE {
    fn rosy_vmin(&self) -> anyhow::Result<RE> {
        Ok(*self)
    }
}

impl RosyVMIN for VE {
    fn rosy_vmin(&self) -> anyhow::Result<RE> {
        if self.is_empty() {
            anyhow::bail!("VMIN called on empty vector");
        }
        Ok(self.iter().copied().fold(f64::INFINITY, f64::min))
    }
}
