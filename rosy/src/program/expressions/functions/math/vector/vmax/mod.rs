//! # VMAX Function
//!
//! Returns the maximum element of a vector.
//!
//! ## Syntax
//!
//! ```text
//! VMAX(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | VE | RE |
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
use crate::transpile::{
    ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
    TranspileableExpr, ValueKind,
};
use anyhow::{Context as AnyhowContext, Error, Result};
use rosy_lib::RosyType;
use std::collections::HashSet;

/// AST node for the `VMAX(expr)` function (vector maximum).
#[derive(Debug)]
pub struct VmaxExpr {
    pub expr: Box<Expr>,
}

impl FromRule for VmaxExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::vmax,
            "Expected vmax rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .context("Missing inner expression for `VMAX`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `VMAX`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `VMAX`"))?,
        );
        Ok(Some(VmaxExpr { expr }))
    }
}
impl Transpile for VmaxExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let inner_output = self.expr.transpile(context)?;

        let serialization = format!("RosyVMAX::rosy_vmax({})?", inner_output.as_ref());

        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
impl TranspileableExpr for VmaxExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let inner_type = self
            .expr
            .type_of(context)
            .context("Failed to determine type of inner expression in VMAX")?;

        match inner_type {
            t if t == RosyType::VE() => Ok(RosyType::RE()),
            _ => anyhow::bail!(
                "VMAX not supported for type: {:?}. Only VE is supported.",
                inner_type
            ),
        }
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
        _resolver: &TypeResolver,
        _ctx: &ScopeContext,
        _deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        ExprRecipe::Literal(RosyType::RE())
    }
}
