//! # Logical NOT Operator
//!
//! Inverts a logical (boolean) value.
//!
//! ## Syntax
//!
//! ```text
//! NOT expr
//! ```
//!
//! ## Supported Types
//!
//! | Input | Result |
//! |-------|--------|
//! | LO | LO |
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

use std::collections::BTreeSet;
use std::collections::HashSet;

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::{ExprFunctionCallResult, TranspileableExpr};
use crate::transpile::{TranspilationInputContext, TranspilationOutput, Transpile, ValueKind};
use anyhow::{Error, Result, anyhow};
use rosy_lib::RosyType;

/// Logical NOT expression (unary operator).
/// Supports both `!x` and `NOT x` syntax.
#[derive(Debug)]
pub struct NotExpr {
    pub operand: Box<Expr>,
}

impl FromRule for NotExpr {
    fn from_rule(_pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::bail!("NotExpr should be created by prefix parser, not FromRule")
    }
}
impl TranspileableExpr for NotExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let operand_type = self.operand.type_of(context)?;
        rosy_lib::UnaryOp::Not
            .return_type(&operand_type)
            .ok_or_else(|| anyhow::anyhow!("Cannot apply NOT to type '{}'!", operand_type))
    }
    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> ExprFunctionCallResult {
        ExprFunctionCallResult::HasFunctionCalls {
            result: resolver.discover_expr_function_calls(&self.operand, ctx),
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
impl Transpile for NotExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let operand_type = self.operand.type_of(context).map_err(|e| vec![e])?;
        if rosy_lib::UnaryOp::Not.return_type(&operand_type).is_none() {
            return Err(vec![anyhow!(
                "Cannot apply NOT to type '{}'!",
                operand_type
            )]);
        }

        let mut errors = Vec::new();
        let mut requested_variables = BTreeSet::new();

        let operand_output = match self.operand.transpile(context) {
            Ok(output) => output,
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling operand of NOT"));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(operand_output.requested_variables.iter().cloned());

        let serialization = format!("RosyNot::rosy_not({})?", operand_output.as_ref());

        if errors.is_empty() {
            Ok(TranspilationOutput {
                serialization,
                requested_variables,
                value_kind: ValueKind::Owned,
            })
        } else {
            Err(errors)
        }
    }
}
