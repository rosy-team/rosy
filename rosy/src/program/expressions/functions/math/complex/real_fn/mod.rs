//! # REAL Function
//!
//! Determines the real part of a value.
//!
//! ## Syntax
//!
//! ```text
//! REAL(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | RE    | RE     |
//! | CM    | RE     |
//! | DA    | DA     |
//! | CD    | DA     |
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

/// AST node for the `REAL(expr)` intrinsic function.
#[derive(Debug)]
pub struct RealFnExpr {
    pub expr: Box<Expr>,
}

impl FromRule for RealFnExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::real_fn,
            "Expected real_fn rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .context("Missing inner expression for `REAL`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `REAL`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `REAL`"))?,
        );
        Ok(Some(RealFnExpr { expr }))
    }
}

impl Transpile for RealFnExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let inner_output = self.expr.transpile(context)?;

        let serialization = format!("RosyREAL::rosy_real({})?", inner_output.as_ref());

        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}

impl TranspileableExpr for RealFnExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        use crate::rosy_lib::intrinsics::real_fn;

        let inner_type = self
            .expr
            .type_of(context)
            .context("Failed to determine type of inner expression in REAL")?;

        real_fn::get_return_type(&inner_type)
            .ok_or_else(|| anyhow::anyhow!("REAL not supported for type: {:?}", inner_type))
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
        let inner = resolver.build_expr_recipe(&self.expr, ctx, deps);
        ExprRecipe::RealFn(Box::new(inner))
    }
}
