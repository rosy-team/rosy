//! # TAN Function
//!
//! Computes the tangent of a value.
//!
//! ## Syntax
//!
//! ```text
//! TAN(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | RE | RE |
//! | VE | VE |
//! | DA | DA |
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

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::rosy_lib::RosyType;
use crate::transpile::{
    ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
    TranspileableExpr, ValueKind,
};
use anyhow::{Context as AnyhowContext, Error, Result};
use std::collections::HashSet;

/// AST node for the `TAN(expr)` intrinsic function.
#[derive(Debug)]
pub struct TanExpr {
    pub expr: Box<Expr>,
}

impl FromRule for TanExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::tan_fn,
            "Expected tan_fn rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .context("Missing inner expression for `TAN`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `TAN`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `TAN`"))?,
        );
        Ok(Some(TanExpr { expr }))
    }
}
impl Transpile for TanExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let inner_type = self.expr.type_of(context).map_err(|e| vec![e])?;

        // Transpile the inner expression
        let inner_output = self.expr.transpile(context)?;

        // Generate the transpiled code
        let serialization = if inner_type == RosyType::RE() {
            format!("{}.tan()", inner_output.as_value())
        } else {
            format!("RosyTAN::rosy_tan({})?", inner_output.as_ref())
        };

        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
impl TranspileableExpr for TanExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        use crate::rosy_lib::intrinsics::tan;

        let inner_type = self
            .expr
            .type_of(context)
            .context("Failed to determine type of inner expression in TAN")?;

        tan::get_return_type(&inner_type)
            .ok_or_else(|| anyhow::anyhow!("TAN not supported for type: {:?}", inner_type))
    }
    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> ExprFunctionCallResult {
        ExprFunctionCallResult::HasFunctionCalls {
            result: resolver.discover_expr_function_calls(&self.expr, ctx),
        }
    }
    fn build_expr_recipe(
        &self,
        resolver: &TypeResolver,
        ctx: &ScopeContext,
        deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        ExprRecipe::TypePreserving(Box::new(resolver.build_expr_recipe(&self.expr, ctx, deps)))
    }
}
