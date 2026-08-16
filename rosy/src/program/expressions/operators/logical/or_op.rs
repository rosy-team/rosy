//! # Logical OR Operator (`OR`)
//!
//! Tests logical disjunction of two boolean values. Returns `LO`.
//!
//! ## Syntax
//!
//! ```text
//! expr OR expr
//! ```
//!
//! ## Type Compatibility
//!
//! | Left | Right | Result | Comment |
//! |------|-------|--------|---------|
//! | LO | LO | LO | Short-circuit logical OR |

use std::collections::BTreeSet;
use std::collections::HashSet;

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::TranspileableExpr;
use crate::transpile::{TranspilationInputContext, TranspilationOutput, Transpile, ValueKind};
use anyhow::{Error, Result, anyhow};
use rosy_lib::RosyType;

/// AST node for the logical OR operator.
#[derive(Debug)]
pub struct OrExpr {
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

impl FromRule for OrExpr {
    fn from_rule(_pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::bail!("OrExpr should be created by infix parser, not FromRule")
    }
}
impl TranspileableExpr for OrExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let left_type = self.left.type_of(context)?;
        let right_type = self.right.type_of(context)?;
        rosy_lib::BinaryOp::Or
            .return_type(&left_type, &right_type)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Cannot apply OR to types '{}' and '{}'!",
                    left_type,
                    right_type
                )
            })
    }
    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> Option<Result<()>> {
        if let Err(e) = resolver.discover_expr_function_calls(&self.left, ctx) {
            return Some(Err(e));
        }
        Some(resolver.discover_expr_function_calls(&self.right, ctx),)
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
impl Transpile for OrExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let left_type = self.left.type_of(context).map_err(|e| vec![e])?;
        let right_type = self.right.type_of(context).map_err(|e| vec![e])?;
        if rosy_lib::BinaryOp::Or
            .return_type(&left_type, &right_type)
            .is_none()
        {
            return Err(vec![anyhow!(
                "Cannot apply OR to types '{}' and '{}'!",
                left_type,
                right_type
            )]);
        }

        let mut errors = Vec::new();
        let mut requested_variables = BTreeSet::new();

        let left_output = match self.left.transpile(context) {
            Ok(output) => output,
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling left-hand side of OR"));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(left_output.requested_variables.iter().cloned());

        let right_output = match self.right.transpile(context) {
            Ok(output) => output,
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling right-hand side of OR"));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(right_output.requested_variables.iter().cloned());

        // Use short-circuit || for LO OR LO
        let serialization = format!(
            "({} || {})",
            left_output.as_value(),
            right_output.as_value()
        );

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
