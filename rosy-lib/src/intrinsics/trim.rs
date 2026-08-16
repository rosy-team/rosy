use crate::ST;

/// Get the return type of TRIM for a given input type.
pub fn get_return_type(input: &crate::RosyType) -> Option<crate::RosyType> {
    match input {
        t if *t == crate::RosyType::ST() => Some(crate::RosyType::ST()),
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
