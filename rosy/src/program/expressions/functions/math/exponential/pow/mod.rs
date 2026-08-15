//! # Power Operator (`^`)
//!
//! Raises a value to a power. Parsed as an infix operator.
//!
//! ## Syntax
//!
//! ```text
//! expr ^ expr
//! ```
//!
//! ## Type Compatibility
//!
//! | Left | Right | Result | Comment |
//! |------|-------|--------|---------|
//! | RE | RE | RE | |
//! | VE | RE | VE | Raise to Real power componentwise |
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
use crate::resolve::{BinaryOpKind, ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::{ExprFunctionCallResult, TranspileableExpr};
use crate::transpile::{TranspilationInputContext, TranspilationOutput, Transpile, ValueKind};
use anyhow::{Error, Result, anyhow};
use rosy_lib::RosyType;

/// AST node for the power/exponentiation operator (`^`).
#[derive(Debug)]
pub struct PowExpr {
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

impl FromRule for PowExpr {
    fn from_rule(_pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        // PowExpr is created by the infix parser, not directly from a rule
        anyhow::bail!("PowExpr should be created by infix parser, not FromRule")
    }
}
impl TranspileableExpr for PowExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        rosy_lib::BinaryOp::Pow
            .return_type(&self.left.type_of(context)?, &self.right.type_of(context)?)
            .ok_or(anyhow::anyhow!(
                "Cannot raise type '{}' to the power of type '{}'!",
                self.left.type_of(context)?,
                self.right.type_of(context)?
            ))
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
        resolver: &TypeResolver,
        ctx: &ScopeContext,
        deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        let left = resolver.build_expr_recipe(&self.left, ctx, deps);
        let right = resolver.build_expr_recipe(&self.right, ctx, deps);
        ExprRecipe::BinaryOp {
            op: BinaryOpKind::Pow,
            left: Box::new(left),
            right: Box::new(right),
        }
    }
}
impl Transpile for PowExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // First, ensure the types are compatible
        let left_type = self.left.type_of(context).map_err(|e| vec![e])?;
        let right_type = self.right.type_of(context).map_err(|e| vec![e])?;
        if rosy_lib::BinaryOp::Pow
            .return_type(&left_type, &right_type)
            .is_none()
        {
            return Err(vec![anyhow!(
                "Cannot raise type '{}' to the power of type '{}'!",
                left_type,
                right_type
            )]);
        }

        // Then, transpile both sides and combine
        let mut errors = Vec::new();
        let mut requested_variables = std::collections::BTreeSet::new();
        let mut left_ref = String::new();
        let mut right_ref = String::new();

        // Transpile left
        match self.left.transpile(context) {
            Ok(output) => {
                left_ref = output.as_ref();
                requested_variables.extend(output.requested_variables);
            }
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling base of exponentiation"));
                }
            }
        }

        // Transpile right
        match self.right.transpile(context) {
            Ok(output) => {
                right_ref = output.as_ref();
                requested_variables.extend(output.requested_variables);
            }
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling exponent of exponentiation"));
                }
            }
        }

        let serialization = format!("RosyPow::rosy_pow({}, {})?", left_ref, right_ref);

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
