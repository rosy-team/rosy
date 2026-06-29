use super::IntrinsicTypeRule;
use crate::rosy_lib::ST;

/// Type registry for TRIM intrinsic function.
///
/// TRIM removes trailing space characters from a string.
pub const TRIM_REGISTRY: &[IntrinsicTypeRule] =
    &[IntrinsicTypeRule::new("ST", "ST", "'  hello  '")];

/// Get the return type of TRIM for a given input type.
pub fn get_return_type(input: &crate::rosy_lib::RosyType) -> Option<crate::rosy_lib::RosyType> {
    match input {
        t if *t == crate::rosy_lib::RosyType::ST() => Some(crate::rosy_lib::RosyType::ST()),
        _ => None,
    }
}

/// Trait for removing trailing spaces from Rosy string types.
pub trait RosyTRIM {
    fn rosy_trim(&self) -> anyhow::Result<ST>;
}

/// TRIM for strings - removes trailing space characters
impl RosyTRIM for ST {
    fn rosy_trim(&self) -> anyhow::Result<ST> {
        Ok(self.trim_end_matches(' ').to_string())
    }
}
