//! # CM() — Complex Number Conversion
//!
//! Converts a value to a complex number (`CM`). When applied to a VE with
//! two elements, creates a complex number from (real, imaginary) components.
//!
//! ## Syntax
//!
//! ```text
//! CM(expr)
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
use anyhow::{Context, Error, Result};
use rosy_lib::RosyType;
use std::collections::HashSet;

/// AST node for the `CM(expr)` type conversion function.
#[derive(Debug)]
pub struct ComplexConvertExpr {
    pub expr: Box<Expr>,
}

impl FromRule for ComplexConvertExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::cm,
            "Expected cm rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner.next().context("Missing inner expression for `CM`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `CM`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `CM`"))?,
        );
        Ok(Some(ComplexConvertExpr { expr }))
    }
}
impl TranspileableExpr for ComplexConvertExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let expr_type = self.expr.type_of(context).map_err(|e| {
            e.context("...while determining type of expression for complex conversion")
        })?;
        let result_type = rosy_lib::intrinsics::cm::get_return_type(&expr_type).ok_or(
            anyhow::anyhow!("Cannot convert type '{}' to 'CM'!", expr_type),
        )?;
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
        ExprRecipe::Literal(RosyType::CM())
    }
}
impl Transpile for ComplexConvertExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // First, ensure the type is convertible to CM
        //
        // Sneaky way to check that the type is compatible :3
        let _ = self.type_of(context).map_err(|e| {
            vec![e.context("...while verifying types of complex conversion expression")]
        })?;

        // Then, transpile the expression
        let inner_output = self.expr.transpile(context).map_err(|e| {
            e.into_iter()
                .map(|err| err.context("...while transpiling expression for CM conversion"))
                .collect::<Vec<Error>>()
        })?;

        // Finally, serialize the conversion
        let serialization = format!(
            "RosyCM::rosy_cm({}).context(\"...while trying to convert to (CM)\")?",
            inner_output.as_ref()
        );
        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
