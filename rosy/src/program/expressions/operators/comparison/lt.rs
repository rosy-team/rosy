//! # Less-Than Operator (`<`)
//!
//! Numeric or lexicographic less-than comparison. Returns `LO`.
//!
//! ## Syntax
//!
//! ```text
//! expr < expr
//! ```
//!
//! ## Type Compatibility
//!
//! | Left | Right | Result | Comment |
//! |------|-------|--------|---------|
//! | RE | RE | LO | Numeric less-than |
//! | ST | ST | LO | Lexicographic ordering |
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/expressions/operators/comparison/lt.rosy"))]
//! ```

use std::collections::BTreeSet;
use std::collections::HashSet;

use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::transpile::TranspileableExpr;
use crate::transpile::{TranspilationInputContext, TranspilationOutput, Transpile, ValueKind};
use anyhow::{Error, Result, anyhow};
use rosy_lib::RosyType;

/// AST node for the less-than operator (`<`).
#[derive(Debug)]
pub struct LtExpr {
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

impl FromRule for LtExpr {
    fn from_rule(_pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::bail!("LtExpr should be created by infix parser, not FromRule")
    }
}
impl TranspileableExpr for LtExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let left_type = self.left.type_of(context)?;
        let right_type = self.right.type_of(context)?;
        rosy_lib::BinaryOp::Lt
            .return_type(&left_type, &right_type)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Cannot compare types '{}' and '{}' with less-than!",
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
        Some(resolver.discover_expr_function_calls(&self.right, ctx))
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
impl Transpile for LtExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let left_type = self.left.type_of(context).map_err(|e| vec![e])?;
        let right_type = self.right.type_of(context).map_err(|e| vec![e])?;
        if rosy_lib::BinaryOp::Lt
            .return_type(&left_type, &right_type)
            .is_none()
        {
            return Err(vec![anyhow!(
                "Cannot compare types '{}' and '{}' with less-than!",
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
                    errors.push(err.context("...while transpiling left-hand side of less-than"));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(left_output.requested_variables.iter().cloned());

        let right_output = match self.right.transpile(context) {
            Ok(output) => output,
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling right-hand side of less-than"));
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
                format!("({} < {})", left_output.as_value(), right_output.as_value())
            }
            _ => format!(
                "RosyLt::rosy_lt({}, {})?",
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
