//! # LO() — Logical Conversion
//!
//! Converts a value to a logical (boolean) type.
//!
//! ## Syntax
//!
//! ```text
//! LO(expr)
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

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::{ExprFunctionCallResult, TranspileableExpr};
use crate::transpile::{TranspilationInputContext, TranspilationOutput, Transpile, ValueKind};
use anyhow::{Context, Error, Result, anyhow};
use rosy_lib::RosyType;
use std::collections::HashSet;

/// AST node for the `LO(expr)` type conversion function.
#[derive(Debug)]
pub struct LogicalConvertExpr {
    pub expr: Box<Expr>,
}

impl FromRule for LogicalConvertExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::lo,
            "Expected lo rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner.next().context("Missing inner expression for `LO`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `LO`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `LO`"))?,
        );
        Ok(Some(LogicalConvertExpr { expr }))
    }
}
impl TranspileableExpr for LogicalConvertExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let expr_type = self.expr.type_of(context)?;
        rosy_lib::intrinsics::lo::get_return_type(&expr_type).ok_or(anyhow::anyhow!(
            "Cannot convert type '{expr_type}' to 'LO'!"
        ))
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
        ExprRecipe::Literal(RosyType::LO())
    }
}
impl Transpile for LogicalConvertExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // First, ensure the type is convertible to LO
        let expr_type = self.expr.type_of(context).map_err(|e| vec![e])?;
        let _ = rosy_lib::intrinsics::lo::get_return_type(&expr_type).ok_or(vec![anyhow!(
            "Cannot convert type '{}' to 'LO'!",
            expr_type
        )])?;

        // Then, transpile the expression
        let inner_output = self.expr.transpile(context).map_err(|e| {
            e.into_iter()
                .map(|err| err.context("...while transpiling expression for LO conversion"))
                .collect::<Vec<Error>>()
        })?;

        // Finally, serialize the conversion
        let serialization = format!("RosyLO::rosy_to_logical({})", inner_output.as_ref());
        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
