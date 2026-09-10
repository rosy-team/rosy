use crate::RosyType;
use crate::{RE, CM, VE, LO, ST, DA, CD};
use crate::core::display::{display_re, RosyDisplay};

pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::ST()),
        t if *t == RosyType::ST() => Some(RosyType::ST()),
        t if *t == RosyType::LO() => Some(RosyType::ST()),
        t if *t == RosyType::CM() => Some(RosyType::ST()),
        t if *t == RosyType::VE() => Some(RosyType::ST()),
        t if *t == RosyType::DA() => Some(RosyType::ST()),
        t if *t == RosyType::CD() => Some(RosyType::ST()),
        _ => None,
    }
}           

/// Trait for converting Rosy data types to strings
pub trait RosyST {
    fn rosy_to_string(self) -> String;
}

/// Convert real numbers to strings.
///
/// COSY `ST` of a RE is the G-format body without the 4-column WRITE pad.
impl RosyST for &RE {
    fn rosy_to_string(self) -> String {
        display_re(*self, 16, 4, 0)
    }
}

/// Convert strings to strings (identity)
impl RosyST for &ST {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

/// Convert booleans to strings
impl RosyST for &LO {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

/// Convert vectors to strings
impl RosyST for &VE {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

/// Convert complex numbers to strings
impl RosyST for &CM {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

/// Convert Differential Algebra (DA) to strings
impl RosyST for &DA {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

/// Convert Complex Differential Algebra (CD) to strings
impl RosyST for &CD {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

#[cfg(test)]
mod tests {
    use super::RosyST;

    #[test]
    fn st_re_matches_cosy_length_without_write_pad() {
        let x = 42.0;
        let s = (&x).rosy_to_string();
        assert!(!s.ends_with(' '), "ST(RE) must not carry WRITE pad: {s:?}");
        assert_eq!(s.len(), 18, "got {s:?}");
    }
}