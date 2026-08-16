//! # Division Operator (`/`)
//!
//! Binary division for numeric, vector, complex, and Taylor series types.
//!
//! ## Syntax
//!
//! ```text
//! expr / expr
//! ```
//!
//! ## Type Compatibility
//!
//! | Left | Right | Result | Comment |
//! |------|-------|--------|---------|
//! | RE | RE | RE | |
//! | RE | CM | CM | |
//! | RE | VE | VE | Divide Real componentwise |
//! | RE | DA | DA | |
//! | RE | CD | CD | |
//! | CM | RE | CM | |
//! | CM | CM | CM | |
//! | CM | DA | CD | |
//! | CM | CD | CD | |
//! | VE | RE | VE | Divide by Real componentwise |
//! | VE | VE | VE | Divide componentwise |
//! | DA | RE | DA | |
//! | DA | CM | CD | |
//! | DA | DA | DA | |
//! | DA | CD | CD | |
//! | CD | RE | CD | |
//! | CD | CM | CD | |
//! | CD | DA | CD | |
//! | CD | CD | CD | |
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/expressions/operators/arithmetic/div.rosy"))]
//! ```

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::TranspileableExpr;
use crate::transpile::{TranspilationInputContext, TranspilationOutput, Transpile, ValueKind};
use anyhow::{Error, Result, anyhow};
use rosy_lib::BinaryOp;
use rosy_lib::RosyType;
use std::collections::{BTreeSet, HashSet};

/// AST node for the binary division operator (`/`).
#[derive(Debug)]
pub struct DivExpr {
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

impl FromRule for DivExpr {
    fn from_rule(_pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        // DivExpr is created by the infix parser, not directly from a rule
        anyhow::bail!("DivExpr should be created by infix parser, not FromRule")
    }
}
impl TranspileableExpr for DivExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let left_type = self.left.type_of(context)?;
        let right_type = self.right.type_of(context)?;
        rosy_lib::BinaryOp::Div
            .return_type(&left_type, &right_type)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Cannot divide types '{}' and '{}' together!",
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
        resolver: &TypeResolver,
        ctx: &ScopeContext,
        deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        let left = resolver.build_expr_recipe(&self.left, ctx, deps);
        let right = resolver.build_expr_recipe(&self.right, ctx, deps);
        ExprRecipe::BinaryOp {
            op: BinaryOp::Div,
            left: Box::new(left),
            right: Box::new(right),
        }
    }
}
impl Transpile for DivExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // First, ensure the types are compatible
        let left_type = self.left.type_of(context).map_err(|e| vec![e])?;
        let right_type = self.right.type_of(context).map_err(|e| vec![e])?;
        if rosy_lib::BinaryOp::Div
            .return_type(&left_type, &right_type)
            .is_none()
        {
            return Err(vec![anyhow!(
                "Cannot divide types '{}' and '{}' together!",
                left_type,
                right_type
            )]);
        }

        // Then, transpile both sides and combine
        let mut errors = Vec::new();
        let mut requested_variables = BTreeSet::new();

        // Transpile left
        let left_output = match self.left.transpile(context) {
            Ok(output) => output,
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling left-hand side of division"));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(left_output.requested_variables.iter().cloned());

        // Transpile right
        let right_output = match self.right.transpile(context) {
            Ok(output) => output,
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling right-hand side of division"));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(right_output.requested_variables.iter().cloned());

        // Direct emission for infallible scalar types
        use rosy_lib::RosyBaseType;
        let serialization = match (&left_type.base_type, &right_type.base_type) {
            (RosyBaseType::RE, RosyBaseType::RE)
                if left_type.dimensions == 0 && right_type.dimensions == 0 =>
            {
                format!("({} / {})", left_output.as_value(), right_output.as_value())
            }
            _ => format!(
                "RosyDiv::rosy_div({}, {})?",
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
