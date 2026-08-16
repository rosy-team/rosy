use crate::RosyType;
use crate::{RE, CM, VE, ST, DA};

/// Get the return type of RE() for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::ST() => Some(RosyType::RE()),
        t if *t == RosyType::CM() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::RE()),
        t if *t == RosyType::DA() => Some(RosyType::RE()),
        _ => None,
    }
}

/// Trait for converting Rosy data types to real (RE).
pub trait RosyREConvert {
    fn rosy_re_convert(&self) -> anyhow::Result<RE>;
}

/// RE -> RE identity
impl RosyREConvert for RE {
    fn rosy_re_convert(&self) -> anyhow::Result<RE> {
        Ok(*self)
    }
}

/// ST -> RE (parse string as f64)
impl RosyREConvert for ST {
    fn rosy_re_convert(&self) -> anyhow::Result<RE> {
        self.trim().parse::<f64>()
            .map_err(|e| anyhow::anyhow!("Failed to convert ST to RE: {}", e))
    }
}

/// CM -> RE (real part)
impl RosyREConvert for CM {
    fn rosy_re_convert(&self) -> anyhow::Result<RE> {
        Ok(self.re)
    }
}

/// VE -> RE (average)
impl RosyREConvert for VE {
    fn rosy_re_convert(&self) -> anyhow::Result<RE> {
        if self.is_empty() {
            anyhow::bail!("RE() called on empty vector");
        }
        Ok(self.iter().sum::<f64>() / self.len() as f64)
    }
}

/// DA -> RE (constant part)
impl RosyREConvert for DA {
    fn rosy_re_convert(&self) -> anyhow::Result<RE> {
        Ok(self.constant_part())
    }
}
