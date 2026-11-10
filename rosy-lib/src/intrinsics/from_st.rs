use std::collections::HashSet;

use anyhow::{Context, Result};

use crate::RosyType;

pub fn can_be_obtained(input: &RosyType) -> bool {
    let registry: HashSet<_> = HashSet::from([
        RosyType::RE(),
        RosyType::ST(),
        RosyType::LO(),
        RosyType::VE(),
        RosyType::ANY(),
    ]);
    registry.contains(input)
}
pub fn from_stdin<T: RosyFromST>() -> Result<T> {
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .context("Failed to read line from stdin!")?;
    let input = input.trim_end().to_string();
    T::rosy_from_st(input)
}

pub trait RosyFromST {
    fn rosy_from_st(st: String) -> Result<Self>
    where
        Self: Sized;
}

/// Parse a single COSY-format float token.
/// Handles Fortran G-format quirks:
///   - Leading/trailing whitespace
///   - `-.NNNN` (no leading zero before dot)
///   - `0.NNNN` prefix
///   - `E` exponent notation like `E-06`, `E+03`
fn parse_cosy_float(token: &str) -> Result<f64> {
    let s = token.trim().replace(['D', 'd'], "E");
    if s.is_empty() {
        return Ok(0.0);
    }
    // Rust's f64 parser can handle most cases, but not `-.NNNN`
    // (missing the zero before the dot). Insert it.
    let normalized = if s.starts_with("-.") {
        format!("-0.{}", &s[2..])
    } else if s.starts_with(".") {
        format!("0.{}", &s[1..])
    } else {
        s.to_string()
    };
    normalized.parse::<f64>().with_context(|| {
        format!(
            "Failed to parse COSY float '{}' (normalized: '{}')",
            token.trim(),
            normalized
        )
    })
}

impl RosyFromST for f64 {
    fn rosy_from_st(st: String) -> Result<Self> {
        // Try parsing the whole string as a single float first
        if let Ok(val) = parse_cosy_float(&st) {
            return Ok(val);
        }
        // If the line contains multiple space-separated values (e.g. a
        // vector line), parse just the first token — COSY reads the first
        // value when READing into a scalar.
        if let Some(first_token) = st.trim().split_whitespace().next() {
            return parse_cosy_float(first_token).context("Failed to parse `ST` to `RE`!");
        }
        parse_cosy_float(&st).context("Failed to parse `ST` to `RE`!")
    }
}

impl RosyFromST for String {
    fn rosy_from_st(st: String) -> Result<Self> {
        Ok(st)
    }
}

impl RosyFromST for bool {
    fn rosy_from_st(st: String) -> Result<Self> {
        match st.trim() {
            "TRUE" => Ok(true),
            "FALSE" => Ok(false),
            _ => Err(anyhow::anyhow!("Failed to parse `ST` to `LO`!")),
        }
    }
}

impl RosyFromST for crate::RosyValue {
    fn rosy_from_st(st: String) -> Result<Self> {
        // One number stays a number. More than one token is the whole
        // line as text: PARSETEXT splits a positions line that way.
        // A text record, like an MFFLD header, stays a string.
        // A declared real still uses `f64::rosy_from_st` and keeps the
        // first token only.
        if st.trim().split_whitespace().nth(1).is_some() {
            return Ok(crate::RosyValue::ST(st));
        }
        match f64::rosy_from_st(st.clone()) {
            Ok(x) => Ok(crate::RosyValue::RE(x)),
            Err(_) => Ok(crate::RosyValue::ST(st)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_read_keeps_a_multi_number_line_as_text() {
        let line = "1.61001e-03 -1.38884e-03 -2.52247e-04".to_string();
        let value = crate::RosyValue::rosy_from_st(line.clone()).unwrap();
        match value {
            crate::RosyValue::ST(text) => assert_eq!(text, line),
            other => panic!("expected ST, got {}", other.kind_name()),
        }
        let scalar = f64::rosy_from_st(line).unwrap();
        assert!((scalar - 1.61001e-03).abs() < 1e-12);
    }

    #[test]
    fn any_read_keeps_a_single_number_as_real() {
        let value = crate::RosyValue::rosy_from_st("50000".to_string()).unwrap();
        match value {
            crate::RosyValue::RE(x) => assert!((x - 50000.0).abs() < 1e-9),
            other => panic!("expected RE, got {}", other.kind_name()),
        }
        let header = crate::RosyValue::rosy_from_st("MFFLD header".to_string()).unwrap();
        assert!(matches!(header, crate::RosyValue::ST(_)));
    }
}

impl RosyFromST for Vec<f64> {
    fn rosy_from_st(st: String) -> Result<Self> {
        // Parse whitespace-separated COSY-format floats from a single line
        let trimmed = st.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        trimmed
            .split_whitespace()
            .map(|token| parse_cosy_float(token))
            .collect::<Result<Vec<f64>>>()
            .context("Failed to parse `ST` to `VE`!")
    }
}
