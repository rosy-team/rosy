//! # VE() — Vector Conversion
//!
//! Converts a value to a vector (`VE`).
//!
//! ## Syntax
//!
//! ```text
//! VE(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | RE    | VE     |
//! | CM    | VE     |
//! | VE    | VE     |
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
use crate::transpile::{ExprFunctionCallResult, TranspileableExpr};
use crate::transpile::{TranspilationInputContext, TranspilationOutput, Transpile, ValueKind};
use anyhow::{Context, Error, Result};
use rosy_lib::RosyType;
use std::collections::HashSet;

/// AST node for the `VE(expr)` type conversion function.
#[derive(Debug)]
pub struct VeConvertExpr {
    pub expr: Box<Expr>,
}

impl FromRule for VeConvertExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::ve_fn,
            "Expected ve_fn rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner.next().context("Missing inner expression for `VE`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `VE`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `VE`"))?,
        );
        Ok(Some(VeConvertExpr { expr }))
    }
}

impl TranspileableExpr for VeConvertExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let expr_type = self
            .expr
            .type_of(context)
            .map_err(|e| e.context("...while determining type of expression for VE conversion"))?;
        let result_type = rosy_lib::unary_return_type("VE", &expr_type).ok_or(anyhow::anyhow!(
            "Cannot convert type '{}' to 'VE'!",
            expr_type
        ))?;
        Ok(result_type)
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
        ExprRecipe::Literal(RosyType::VE())
    }
}

impl Transpile for VeConvertExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Verify type compatibility
        let _ = self
            .type_of(context)
            .map_err(|e| vec![e.context("...while verifying types of VE conversion expression")])?;

        let inner_output = self.expr.transpile(context).map_err(|e| {
            e.into_iter()
                .map(|err| err.context("...while transpiling expression for VE conversion"))
                .collect::<Vec<Error>>()
        })?;

        let serialization = format!(
            "RosyVEConvert::rosy_ve_convert({}).context(\"...while trying to convert to (VE)\")?",
            inner_output.as_ref()
        );
        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
