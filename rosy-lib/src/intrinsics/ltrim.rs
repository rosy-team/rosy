use crate::ST;

/// Get the return type of LTRIM for a given input type.
pub fn get_return_type(input: &crate::RosyType) -> Option<crate::RosyType> {
    match input {
        t if *t == crate::RosyType::ST() => Some(crate::RosyType::ST()),
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
