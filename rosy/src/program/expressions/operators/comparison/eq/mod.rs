//! # Equality Operator (`=`)
//!
//! Tests two values for equality. Returns `LO` (logical/boolean).
//!
//! ## Syntax
//!
//! ```text
//! expr = expr
//! ```
//!
//! ## Type Compatibility
//!
//! | Left | Right | Result | Comment |
//! |------|-------|--------|---------|
//! | RE | RE | LO | Equality with epsilon tolerance |
//! | ST | ST | LO | String equality |
//! | LO | LO | LO | Logical equality |
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

/// AST node for the equality operator (`=`).
#[derive(Debug)]
pub struct EqExpr {
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

impl FromRule for EqExpr {
    fn from_rule(_pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        // EqExpr is created by the infix parser, not directly from a rule
        anyhow::bail!("EqExpr should be created by infix parser, not FromRule")
    }
}
impl TranspileableExpr for EqExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let left_type = self.left.type_of(context)?;
        let right_type = self.right.type_of(context)?;
        rosy_lib::BinaryOp::Eq
            .return_type(&left_type, &right_type)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Cannot compare types '{}' and '{}' for equality!",
                    left_type,
                    right_type
                )
            })
    }
    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> ExprFunctionCallResult {
        if let Err(e) = resolver.discover_expr_function_calls(&self.left, ctx) {
            return ExprFunctionCallResult::HasFunctionCalls { result: Err(e) };
        }
        ExprFunctionCallResult::HasFunctionCalls {
            result: resolver.discover_expr_function_calls(&self.right, ctx),
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
impl Transpile for EqExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // First, ensure the types are compatible
        let left_type = self.left.type_of(context).map_err(|e| vec![e])?;
        let right_type = self.right.type_of(context).map_err(|e| vec![e])?;
        if rosy_lib::BinaryOp::Eq
            .return_type(&left_type, &right_type)
            .is_none()
        {
            return Err(vec![anyhow!(
                "Cannot compare types '{}' and '{}' for equality!",
                left_type,
                right_type
            )]);
        }

        // Then, transpile both sides and combine
        let mut errors = Vec::new();
        let mut requested_variables = BTreeSet::new();

        let left_output = match self.left.transpile(context) {
            Ok(output) => output,
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling left-hand side of equality"));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(left_output.requested_variables.iter().cloned());

        let right_output = match self.right.transpile(context) {
            Ok(output) => output,
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling right-hand side of equality"));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(right_output.requested_variables.iter().cloned());

        use rosy_lib::RosyBaseType;
        let serialization = match (&left_type.base_type, &right_type.base_type) {
            (RosyBaseType::RE, RosyBaseType::RE) | (RosyBaseType::ST, RosyBaseType::ST)
                if left_type.dimensions == 0 && right_type.dimensions == 0 =>
            {
                format!(
                    "({} == {})",
                    left_output.as_value(),
                    right_output.as_value()
                )
            }
            _ => format!(
                "RosyEq::rosy_eq({}, {})?",
                left_output.as_ref(),
                right_output.as_ref()
            ),
        };

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
