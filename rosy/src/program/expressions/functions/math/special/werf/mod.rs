//! # WERF Function (Faddeeva / Complex Error Function w)
//!
//! Computes the Faddeeva function w(z) = exp(-z^2) * erfc(-iz).
//!
//! ## Syntax
//!
//! ```text
//! WERF(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | CM | CM |
//! | CD | CD |
//!
//! ## Rosy Example
//! ```text
#![doc = include_str!("test.rosy")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("rosy_output.txt")]
//! ```
//! ## COSY Example
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

/// AST node for the `WERF(expr)` intrinsic function (Faddeeva function).
#[derive(Debug)]
pub struct WerfExpr {
    pub expr: Box<Expr>,
}

impl FromRule for WerfExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::werf_fn,
            "Expected werf_fn rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .context("Missing inner expression for `WERF`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `WERF`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `WERF`"))?,
        );
        Ok(Some(WerfExpr { expr }))
    }
}
impl Transpile for WerfExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Transpile the inner expression
        let inner_output = self.expr.transpile(context)?;

        // All types route through the trait
        let serialization = format!("RosyWERF::rosy_werf({})?", inner_output.as_ref());

        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
impl TranspileableExpr for WerfExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        use crate::rosy_lib::intrinsics::werf;

        let inner_type = self
            .expr
            .type_of(context)
            .context("Failed to determine type of inner expression in WERF")?;

        werf::get_return_type(&inner_type)
            .ok_or_else(|| anyhow::anyhow!("WERF not supported for type: {:?}", inner_type))
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
