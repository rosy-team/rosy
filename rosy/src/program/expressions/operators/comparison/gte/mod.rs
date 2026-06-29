//! # Greater-Than-or-Equal Operator (`>=`)
//!
//! Numeric or lexicographic greater-than-or-equal comparison. Returns `LO`.
//!
//! ## Syntax
//!
//! ```text
//! expr >= expr
//! ```
//!
//! ## Type Compatibility
//!
//! | Left | Right | Result | Comment |
//! |------|-------|--------|---------|
//! | RE | RE | LO | Numeric greater-than-or-equal |
//! | ST | ST | LO | Lexicographic ordering |
//!
//! ## Rosy Example
//! ```text
#![doc = include_str!("test.rosy")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("rosy_output.txt")]
//! ```

use std::collections::BTreeSet;
use std::collections::HashSet;

use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::rosy_lib::RosyType;
use crate::transpile::{ExprFunctionCallResult, TranspileableExpr};
use crate::transpile::{TranspilationInputContext, TranspilationOutput, Transpile, ValueKind};
use anyhow::{Error, Result, anyhow};

/// AST node for the greater-than-or-equal operator (`>=`).
#[derive(Debug)]
pub struct GteExpr {
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

impl FromRule for GteExpr {
    fn from_rule(_pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::bail!("GteExpr should be created by infix parser, not FromRule")
    }
}
impl TranspileableExpr for GteExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let left_type = self.left.type_of(context)?;
        let right_type = self.right.type_of(context)?;
        crate::rosy_lib::operators::gte::get_return_type(&left_type, &right_type).ok_or_else(|| {
            anyhow::anyhow!(
                "Cannot compare types '{}' and '{}' with greater-than-or-equal!",
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
impl Transpile for GteExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let left_type = self.left.type_of(context).map_err(|e| vec![e])?;
        let right_type = self.right.type_of(context).map_err(|e| vec![e])?;
        if crate::rosy_lib::operators::gte::get_return_type(&left_type, &right_type).is_none() {
            return Err(vec![anyhow!(
                "Cannot compare types '{}' and '{}' with greater-than-or-equal!",
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
                    errors.push(
                        err.context("...while transpiling left-hand side of greater-than-or-equal"),
                    );
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(left_output.requested_variables.iter().cloned());

        let right_output =
            match self.right.transpile(context) {
                Ok(output) => output,
                Err(mut e) => {
                    for err in e.drain(..) {
                        errors.push(err.context(
                            "...while transpiling right-hand side of greater-than-or-equal",
                        ));
                    }
                    TranspilationOutput::default()
                }
            };
        requested_variables.extend(right_output.requested_variables.iter().cloned());

        use crate::rosy_lib::RosyBaseType;
        let serialization = match (&left_type.base_type, &right_type.base_type) {
            (RosyBaseType::RE, RosyBaseType::RE) | (RosyBaseType::ST, RosyBaseType::ST)
                if left_type.dimensions == 0 && right_type.dimensions == 0 =>
            {
                format!(
                    "({} >= {})",
                    left_output.as_value(),
                    right_output.as_value()
                )
            }
            _ => format!(
                "RosyGte::rosy_gte({}, {})?",
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
