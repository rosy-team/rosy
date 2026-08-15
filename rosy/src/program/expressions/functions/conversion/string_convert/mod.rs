//! # ST() — String Conversion
//!
//! Converts any value to its string representation.
//! Commonly used in `WRITE` statements for output formatting.
//!
//! ## Syntax
//!
//! ```text
//! ST(expr)
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

use std::collections::HashSet;

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::{
    ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
    TranspileableExpr, ValueKind,
};
use anyhow::{Context, Error, Result, anyhow};
use rosy_lib::RosyType;

/// AST node for the `ST(expr)` type conversion function.
#[derive(Debug)]
pub struct StringConvertExpr {
    pub expr: Box<Expr>,
}

impl FromRule for StringConvertExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::st,
            "Expected st rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner.next().context("Missing inner expression for `ST`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `ST`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `ST`"))?,
        );
        Ok(Some(StringConvertExpr { expr }))
    }
}

impl TranspileableExpr for StringConvertExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let expr_type = self.expr.type_of(context)?;
        rosy_lib::unary_return_type("ST", &expr_type).ok_or(anyhow::anyhow!(
            "Cannot convert type '{expr_type}' to 'ST'!"
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
        ExprRecipe::Literal(RosyType::ST())
    }
}
impl Transpile for StringConvertExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        string_convert_transpile_helper(&self.expr, context)
    }
}

pub fn string_convert_transpile_helper(
    expr: &Expr,
    context: &mut TranspilationInputContext,
) -> Result<TranspilationOutput, Vec<Error>> {
    // First, ensure the type is convertible to ST
    let expr_type = expr.type_of(context).map_err(|e| vec![e])?;
    let _ = rosy_lib::unary_return_type("ST", &expr_type).ok_or(vec![anyhow!(
        "Cannot convert type '{}' to 'ST'!",
        expr_type
    )])?;

    // Then, transpile the expression
    let inner_output = expr.transpile(context).map_err(|e| {
        e.into_iter()
            .map(|err| err.context("...while transpiling expression for STRING conversion"))
            .collect::<Vec<Error>>()
    })?;

    // Finally, serialize the conversion
    let serialization = format!("RosyST::rosy_to_string({})", inner_output.as_ref());
    Ok(TranspilationOutput {
        serialization,
        requested_variables: inner_output.requested_variables,
        value_kind: ValueKind::Owned,
    })
}
