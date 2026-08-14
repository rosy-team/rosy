//! # POSITION Function
//!
//! Returns the 1-based index of the first occurrence of a substring,
//! or 0 if not found. Matches COSY INFINITY behavior.
//!
//! ## Syntax
//!
//! ```text
//! POSITION(haystack, needle)
//! ```
//!
//! ## Type Compatibility
//!
//! | Haystack | Needle | Result |
//! |----------|--------|--------|
//! | ST | ST | RE |

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::{
    ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
    TranspileableExpr, ValueKind,
};
use anyhow::{Context as AnyhowContext, Error, Result};
use rosy_lib::RosyType;
use std::collections::HashSet;

/// AST node for the `POSITION(haystack, needle)` system function.
#[derive(Debug)]
pub struct PositionExpr {
    pub haystack: Box<Expr>,
    pub needle: Box<Expr>,
}

impl FromRule for PositionExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::position,
            "Expected position rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let haystack_pair = inner
            .next()
            .context("Missing haystack expression for `POSITION`!")?;
        let haystack = Box::new(
            Expr::from_rule(haystack_pair)
                .context("Failed to build haystack expression for `POSITION`")?
                .ok_or_else(|| anyhow::anyhow!("Expected haystack expression for `POSITION`"))?,
        );
        let needle_pair = inner
            .next()
            .context("Missing needle expression for `POSITION`!")?;
        let needle = Box::new(
            Expr::from_rule(needle_pair)
                .context("Failed to build needle expression for `POSITION`")?
                .ok_or_else(|| anyhow::anyhow!("Expected needle expression for `POSITION`"))?,
        );
        Ok(Some(PositionExpr { haystack, needle }))
    }
}
impl Transpile for PositionExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let haystack_output = self.haystack.transpile(context)?;
        let needle_output = self.needle.transpile(context)?;

        let mut requested_variables = haystack_output.requested_variables.clone();
        requested_variables.extend(needle_output.requested_variables.iter().cloned());

        let serialization = format!(
            "RosyPOSITION::rosy_position({}, {})",
            haystack_output.as_ref(),
            needle_output.as_ref()
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
impl TranspileableExpr for PositionExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        use rosy_lib::intrinsics::position;

        let haystack_type = self
            .haystack
            .type_of(context)
            .context("Failed to determine type of haystack in POSITION")?;
        let needle_type = self
            .needle
            .type_of(context)
            .context("Failed to determine type of needle in POSITION")?;

        position::get_return_type(&haystack_type, &needle_type).ok_or_else(|| {
            anyhow::anyhow!(
                "POSITION requires (ST, ST) arguments, got ({}, {})",
                haystack_type,
                needle_type
            )
        })
    }
    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> ExprFunctionCallResult {
        if let Err(e) = resolver.discover_expr_function_calls(&self.haystack, ctx) {
            return ExprFunctionCallResult::HasFunctionCalls { result: Err(e) };
        }
        ExprFunctionCallResult::HasFunctionCalls {
            result: resolver.discover_expr_function_calls(&self.needle, ctx),
        }
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
