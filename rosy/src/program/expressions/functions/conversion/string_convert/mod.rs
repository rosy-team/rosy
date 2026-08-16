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

use anyhow::{anyhow, Error, Result};
use crate::program::expressions::Expr;
use crate::transpile::{
    TranspilationInputContext, TranspilationOutput, Transpile, TranspileableExpr, ValueKind,
};

/// Shared emit path for `ST(expr)` — used by the intrinsic and by `WRITE`.
pub fn string_convert_transpile_helper(
    expr: &Expr,
    context: &mut TranspilationInputContext,
) -> Result<TranspilationOutput, Vec<Error>> {
    let expr_type = expr.type_of(context).map_err(|e| vec![e])?;
    let _ = rosy_lib::unary_return_type("ST", &expr_type).ok_or(vec![anyhow!(
        "Cannot convert type '{}' to 'ST'!",
        expr_type
    )])?;

    let inner_output = expr.transpile(context).map_err(|e| {
        e.into_iter()
            .map(|err| err.context("...while transpiling expression for STRING conversion"))
            .collect::<Vec<Error>>()
    })?;

    let serialization = format!("RosyST::rosy_to_string({})", inner_output.as_ref());
    Ok(TranspilationOutput {
        serialization,
        requested_variables: inner_output.requested_variables,
        value_kind: ValueKind::Owned,
    })
}

