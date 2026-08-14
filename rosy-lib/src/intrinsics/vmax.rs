use crate::{RE, VE};

/// Trait for computing the maximum element of Rosy vector types.
pub trait RosyVMAX {
    fn rosy_vmax(&self) -> anyhow::Result<RE>;
}

/// VMAX for vectors - returns the maximum element
impl RosyVMAX for VE {
    fn rosy_vmax(&self) -> anyhow::Result<RE> {
        if self.is_empty() {
            anyhow::bail!("VMAX called on empty vector");
        }
        Ok(self.iter().copied().fold(f64::NEG_INFINITY, f64::max))
    }
}
