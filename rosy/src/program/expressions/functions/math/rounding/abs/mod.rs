//! # ABS Function
//!
//! Computes the absolute value of a value.
//!
//! ## Syntax
//!
//! ```text
//! ABS(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | RE | RE |
//! | CM | RE |
//! | VE | RE |
//! | DA | RE |
//! | CD | RE |
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

/// AST node for the `ABS(expr)` intrinsic function (absolute value).
#[derive(Debug)]
pub struct AbsExpr {
    pub expr: Box<Expr>,
}

impl FromRule for AbsExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::abs_fn,
            "Expected abs_fn rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .context("Missing inner expression for `ABS`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `ABS`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `ABS`"))?,
        );
        Ok(Some(AbsExpr { expr }))
    }
}
impl Transpile for AbsExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let inner_type = self.expr.type_of(context).map_err(|e| vec![e])?;

        let inner_output = self.expr.transpile(context)?;

        let serialization = if inner_type == RosyType::RE() {
            format!("{}.abs()", inner_output.as_value())
        } else {
            format!("RosyABS::rosy_abs({})?", inner_output.as_ref())
        };

        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
impl TranspileableExpr for AbsExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        
        let inner_type = self
            .expr
            .type_of(context)
            .context("Failed to determine type of inner expression in ABS")?;

        rosy_lib::unary_return_type("ABS", &inner_type)
            .ok_or_else(|| anyhow::anyhow!("ABS not supported for type: {:?}", inner_type))
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
