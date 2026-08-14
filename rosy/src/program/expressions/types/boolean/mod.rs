//! # LO — Boolean Literal
//!
//! Logical literals `TRUE` and `FALSE`. Produce the `LO` type.
//!
//! ## Syntax
//!
//! ```text
//! TRUE
//! FALSE
//! ```
//!
//! ## Rosy Example
//! ```text
#![doc = include_str!("test.rosy")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("rosy_output.txt")]
//! ```
//! ## COSY INFINITY Example
//! ```text
#![doc = include_str!("test.fox")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("cosy_output.txt")]
//! ```

use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use anyhow::{Error, Result, bail};
use std::collections::{BTreeSet, HashSet};

use crate::{
    ast::{FromRule, Rule},
    transpile::{
        ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableExpr, ValueKind,
    },
};
use rosy_lib::RosyType;

impl FromRule for bool {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::boolean,
            "Expected boolean rule, got {:?}",
            pair.as_rule()
        );
        let b = match pair.as_str() {
            "TRUE" => true,
            "FALSE" => false,
            _ => bail!("Unexpected boolean value: {}", pair.as_str()),
        };
        Ok(Some(b))
    }
}
impl TranspileableExpr for bool {
    fn type_of(&self, _context: &TranspilationInputContext) -> Result<RosyType> {
        Ok(RosyType::LO())
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
        ExprRecipe::Literal(RosyType::LO())
    }
}
impl Transpile for bool {
    fn transpile(
        &self,
        _context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        Ok(TranspilationOutput {
            serialization: format!("{}", self),
            requested_variables: BTreeSet::new(),
            value_kind: ValueKind::Owned,
        })
    }
}
