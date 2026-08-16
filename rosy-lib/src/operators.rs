pub mod add;
pub mod sub;
pub mod mult;
pub mod div;
pub mod pow;
pub mod extract;
pub mod concat;
pub mod eq;
pub mod neq;
pub mod lt;
pub mod gt;
pub mod lte;
pub mod gte;
pub mod not;
pub mod and;
pub mod or;

pub use add::RosyAdd;
pub use sub::RosySub;
pub use mult::RosyMult;
pub use div::RosyDiv;
pub use pow::RosyPow;
pub use concat::RosyConcat;
pub use extract::RosyExtract;
pub use eq::RosyEq;
pub use neq::RosyNeq;
pub use lt::RosyLt;
pub use gt::RosyGt;
pub use lte::RosyLte;
pub use gte::RosyGte;
pub use not::RosyNot;
pub use and::RosyAnd;
pub use or::RosyOr;

use std::collections::HashMap;
use crate::{RosyType, RosyBaseType};

/// Defines a type compatibility rule for an operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeRule {
    /// Left-hand side type
    pub lhs: &'static str,
    /// Right-hand side type
    pub rhs: &'static str,
    /// Result type
    pub result: &'static str,
    /// Test values for lhs and rhs
    pub lhs_test_val: &'static str,
    pub rhs_test_val: &'static str,
    /// Optional comment for documentation
    pub comment: &'static str,
}

impl TypeRule {
    /// Create a new type rule without a comment.
    pub const fn new(
        lhs: &'static str,
        rhs: &'static str,
        result: &'static str,
        lhs_test_val: &'static str,
        rhs_test_val: &'static str
    ) -> Self {
        Self { lhs, rhs, result, lhs_test_val, rhs_test_val, comment: "" }
    }
    
    /// Create a new type rule with a comment.
    pub const fn with_comment(
        lhs: &'static str,
        rhs: &'static str,
        result: &'static str,
        lhs_test_val: &'static str,
        rhs_test_val: &'static str,
        comment: &'static str
    ) -> Self {
        Self { lhs, rhs, result, lhs_test_val, rhs_test_val, comment }
    }
}

/// Convert a type string to RosyType.
/// 
/// This is used by operator registries to convert type rule strings
/// into actual RosyType instances for runtime lookups.
pub fn type_from_str(s: &str) -> RosyType {
    // Support dimensioned types like "DA1" → (DA 1D), "DA2" → (DA 2D)
    if s.len() >= 3 {
        if let Some(dim_str) = s.strip_prefix("DA") {
            if let Ok(dims) = dim_str.parse::<usize>() {
                return RosyType::new(RosyBaseType::DA, dims);
            }
        }
        if let Some(dim_str) = s.strip_prefix("CD") {
            if let Ok(dims) = dim_str.parse::<usize>() {
                return RosyType::new(RosyBaseType::CD, dims);
            }
        }
    }
    match s {
        "RE" => RosyType::new(RosyBaseType::RE, 0),
        "ST" => RosyType::new(RosyBaseType::ST, 0),
        "LO" => RosyType::new(RosyBaseType::LO, 0),
        "CM" => RosyType::new(RosyBaseType::CM, 0),
        "VE" => RosyType::new(RosyBaseType::VE, 0),
        "DA" => RosyType::new(RosyBaseType::DA, 0),
        "CD" => RosyType::new(RosyBaseType::CD, 0),
        _ => panic!("Unknown type: {}", s),
    }
}

/// Build a type compatibility registry from a slice of TypeRules.
/// 
/// This is a helper function used by operators to convert their const
/// TypeRule arrays into runtime HashMap lookups.
pub fn build_type_registry(rules: &[TypeRule]) -> HashMap<(RosyType, RosyType), RosyType> {
    let mut m = HashMap::new();
    for rule in rules {
        m.insert(
            (type_from_str(rule.lhs), type_from_str(rule.rhs)), 
            type_from_str(rule.result)
        );
    }
    m
}