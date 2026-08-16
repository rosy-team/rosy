//! # ST — String Literal
//!
//! String literals enclosed in single quotes. Escaped single quotes
//! are written as `''` inside the string.
//!
//! ## Syntax
//!
//! ```text
//! 'hello world'
//! 'it''s escaped'
//! ```
//!
//! All string literals produce the `ST` type.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/expressions/types/string.rosy"))]
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

impl FromRule for String {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::string,
            "Expected string rule, got {:?}",
            pair.as_rule()
        );
        let s = pair.as_str();

        // Remove the surrounding quotes
        let s = &s[1..s.len() - 1];

        // Handle escaped single quotes: '' -> '
        let s = s.replace("''", "'");

        Ok(Some(s.to_string()))
    }
}
impl TranspileableExpr for String {
    fn type_of(&self, _context: &TranspilationInputContext) -> Result<RosyType> {
        Ok(RosyType::ST())
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
        ExprRecipe::Literal(RosyType::ST())
    }
}
impl Transpile for String {
    fn transpile(
        &self,
        _context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        Ok(TranspilationOutput {
            serialization: format!("String::from(\"{}\")", self),
            requested_variables: BTreeSet::new(),
            value_kind: ValueKind::Owned,
        })
    }
}
