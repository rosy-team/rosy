//! # RE — Numeric Literal
//!
//! Real number literals parsed as `f64`. The `FromRule`, `TranspileableExpr`, and
//! `Transpile` traits are implemented directly on `f64`.
//!
//! ## Syntax
//!
//! ```text
//! 3.14
//! 42
//! -7
//! 1.23E-4
//! ```
//!
//! All numeric literals produce the `RE` type.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/expressions/types/number.rosy"))]
//! ```

use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use anyhow::{Error, Result};
use std::collections::{BTreeSet, HashSet};

use crate::{
    ast::{FromRule, Rule},
    transpile::{
        ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableExpr, ValueKind,
    },
};
use rosy_lib::RosyType;

impl FromRule for f64 {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::number,
            "Expected number rule, got {:?}",
            pair.as_rule()
        );
        // Accept Fortran-style `D/d` exponents (e.g. `1.66053873D-27`)
        // alongside `E/e`. Rust's `f64::from_str` only knows about the
        // latter, so normalize before parsing.
        let raw = pair.as_str();
        let normalized = if raw.contains(['d', 'D']) {
            raw.replace('D', "E").replace('d', "e")
        } else {
            raw.to_string()
        };
        let n = normalized.parse::<f64>()?;
        Ok(Some(n))
    }
}
impl TranspileableExpr for f64 {
    fn type_of(&self, _context: &TranspilationInputContext) -> Result<RosyType> {
        Ok(RosyType::RE())
    }
    fn discover_expr_function_calls(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &ScopeContext,
    ) -> ExprFunctionCallResult {
        ExprFunctionCallResult::NoFunctionCalls
    }
    fn build_expr_recipe(
        &self,
        _resolver: &TypeResolver,
        _ctx: &ScopeContext,
        _deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        ExprRecipe::Literal(RosyType::RE())
    }
}
impl Transpile for f64 {
    fn transpile(
        &self,
        _context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        Ok(TranspilationOutput {
            serialization: format!("{}f64", self),
            requested_variables: BTreeSet::new(),
            value_kind: ValueKind::Owned,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{CosyParser, Rule};
    use pest::Parser;

    /// Parse a numeric literal source through the grammar and assert that the
    /// rule matches the entire input — i.e. nothing was left unconsumed.
    fn must_parse_as_number(src: &str) {
        let mut pairs = CosyParser::parse(Rule::number, src)
            .unwrap_or_else(|e| panic!("grammar rejected '{src}': {e}"));
        let pair = pairs.next().expect("no pair");
        assert_eq!(
            pair.as_str(),
            src,
            "rule consumed only '{}' of '{}' (expected full match)",
            pair.as_str(),
            src
        );
    }

    #[test]
    fn number_parses_integer_and_decimal() {
        must_parse_as_number("0");
        must_parse_as_number("42");
        must_parse_as_number("-7");
        must_parse_as_number("3.14");
        must_parse_as_number("-2.71828");
    }

    /// Scientific notation must match in ONE rule application, otherwise
    /// `1.66E-7` decays to `1.66` followed by an unbound `E-7` expression
    /// (the latter being treated as an undeclared variable). Checks both
    /// uppercase / lowercase E and explicit / implicit exponent sign.
    #[test]
    fn number_parses_scientific_notation() {
        must_parse_as_number("1e5");
        must_parse_as_number("1E5");
        must_parse_as_number("1.5e10");
        must_parse_as_number("1.66E-7");
        must_parse_as_number("-2.5e+10");
        must_parse_as_number("6.022E+23");
    }

    /// cosy.fox uses Fortran's `D/d` exponent convention to mark
    /// double-precision literals (e.g. `1.66053873D-27` for the AMU).
    /// The parser accepts this form too, normalizing to `E/e` before
    /// the actual `f64::from_str`.
    #[test]
    fn number_parses_fortran_d_exponent() {
        must_parse_as_number("1.66053873D-27");
        must_parse_as_number("1.602176462D-19");
        must_parse_as_number("2.99792458D8");
        must_parse_as_number("5.485799110d-4");
        must_parse_as_number("1D-6");
    }
}
