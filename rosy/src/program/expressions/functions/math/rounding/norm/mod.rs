//! # NORM Function
//!
//! Computes the norm of a value.
//!
//! ## Syntax
//!
//! ```text
//! NORM(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | VE    | RE     |
//! | DA    | RE     |
//! | CD    | RE     |
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

/// AST node for the `NORM(expr)` intrinsic function.
#[derive(Debug)]
pub struct NormExpr {
    pub expr: Box<Expr>,
}

impl FromRule for NormExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::norm_fn,
            "Expected norm_fn rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .context("Missing inner expression for `NORM`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `NORM`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `NORM`"))?,
        );
        Ok(Some(NormExpr { expr }))
    }
}

impl Transpile for NormExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let inner_type = self.expr.type_of(context).map_err(|e| vec![e])?;

        let inner_output = self.expr.transpile(context)?;

        let serialization = if inner_type == RosyType::RE() {
            format!("{}.abs()", inner_output.as_value())
        } else {
            format!("RosyNORM::rosy_norm({})?", inner_output.as_ref())
        };

        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}

impl TranspileableExpr for NormExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        
        let inner_type = self
            .expr
            .type_of(context)
            .context("Failed to determine type of inner expression in NORM")?;

        rosy_lib::unary_return_type("NORM", &inner_type)
            .ok_or_else(|| anyhow::anyhow!("NORM not supported for type: {:?}", inner_type))
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
