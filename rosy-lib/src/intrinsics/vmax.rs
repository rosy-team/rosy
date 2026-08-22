use crate::{RE, RosyType, VE};

/// VMAX: VE → RE
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    if *input == RosyType::VE() || *input == RosyType::RE() {
        Some(RosyType::RE())
    } else {
        None
    }
}

/// Trait for computing the maximum element of Rosy vector types.
pub trait RosyVMAX {
    fn rosy_vmax(&self) -> anyhow::Result<RE>;
}

/// VMAX for vectors - returns the maximum element
impl RosyVMAX for RE {
    fn rosy_vmax(&self) -> anyhow::Result<RE> {
        Ok(*self)
    }
}

impl RosyVMAX for VE {
    fn rosy_vmax(&self) -> anyhow::Result<RE> {
        if self.is_empty() {
            anyhow::bail!("VMAX called on empty vector");
        }
        Ok(self.iter().copied().fold(f64::NEG_INFINITY, f64::max))
    }
}
