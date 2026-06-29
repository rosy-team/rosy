use super::IntrinsicTypeRule;
use crate::rosy_lib::ST;

/// Type registry for LTRIM intrinsic function.
///
/// LTRIM removes leading space characters from a string.
pub const LTRIM_REGISTRY: &[IntrinsicTypeRule] =
    &[IntrinsicTypeRule::new("ST", "ST", "'  hello  '")];

/// Get the return type of LTRIM for a given input type.
pub fn get_return_type(input: &crate::rosy_lib::RosyType) -> Option<crate::rosy_lib::RosyType> {
    match input {
        t if *t == crate::rosy_lib::RosyType::ST() => Some(crate::rosy_lib::RosyType::ST()),
        _ => None,
    }
}

/// Trait for removing leading spaces from Rosy string types.
pub trait RosyLTRIM {
    fn rosy_ltrim(&self) -> anyhow::Result<ST>;
}

/// LTRIM for strings - removes leading space characters
impl RosyLTRIM for ST {
    fn rosy_ltrim(&self) -> anyhow::Result<ST> {
        Ok(self.trim_start_matches(' ').to_string())
    }
}
