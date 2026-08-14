//! # ACOS Function
//!
//! Computes the arccosine of a value.
//!
//! ## Syntax
//!
//! ```text
//! ACOS(expr)
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
use crate::transpile::{
    ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
    TranspileableExpr, ValueKind,
};
use anyhow::{Context as AnyhowContext, Error, Result};
use rosy_lib::RosyType;
use std::collections::HashSet;

/// AST node for the `ACOS(expr)` intrinsic function.
#[derive(Debug)]
pub struct AcosExpr {
    pub expr: Box<Expr>,
}

impl FromRule for AcosExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::acos_fn,
            "Expected acos_fn rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .context("Missing inner expression for `ACOS`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `ACOS`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `ACOS`"))?,
        );
        Ok(Some(AcosExpr { expr }))
    }
}
impl Transpile for AcosExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let inner_type = self.expr.type_of(context).map_err(|e| vec![e])?;

        let inner_output = self.expr.transpile(context)?;

        let serialization = if inner_type == RosyType::RE() {
            format!("{}.acos()", inner_output.as_value())
        } else {
            format!("RosyACOS::rosy_acos({})?", inner_output.as_ref())
        };

        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
impl TranspileableExpr for AcosExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        use rosy_lib::intrinsics::acos;

        let inner_type = self
            .expr
            .type_of(context)
            .context("Failed to determine type of inner expression in ACOS")?;

        acos::get_return_type(&inner_type)
            .ok_or_else(|| anyhow::anyhow!("ACOS not supported for type: {:?}", inner_type))
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
