//! # Extraction Operator (`|`)
//!
//! Extracts elements or sub-ranges from vectors, strings, DA, and CD values.
//!
//! ## Syntax
//!
//! ```text
//! expr | expr
//! ```
//!
//! ## Type Compatibility
//!
//! | Left | Right | Result | Comment |
//! |------|-------|--------|---------|
//! | ST | RE | ST | Extract i-th character |
//! | ST | VE | ST | Extract substring by range |
//! | CM | RE | RE | Extract real part |
//! | VE | RE | RE | Extract i-th component |
//! | VE | VE | VE | Extract subvector by range |
//! | DA | RE | RE | Extract DA coefficient by flat index |
//! | DA | VE | RE | Extract DA coefficient by exponent vector |
//! | CD | RE | CM | Extract CD coefficient by flat index |
//! | CD | VE | CM | Extract CD coefficient by exponent vector |
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

use std::collections::{BTreeSet, HashSet};

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::resolve::{BinaryOpKind, ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::{ExprFunctionCallResult, TranspileableExpr};
use crate::transpile::{TranspilationInputContext, TranspilationOutput, Transpile, ValueKind};
use anyhow::{Error, Result};
use rosy_lib::RosyType;

/// AST node for the extraction operator (`|`).
#[derive(Debug)]
pub struct ExtractExpr {
    pub object: Box<Expr>,
    pub index: Box<Expr>,
}

impl FromRule for ExtractExpr {
    fn from_rule(_pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        // ExtractExpr is created by the infix parser, not directly from a rule
        anyhow::bail!("ExtractExpr should be created by infix parser, not FromRule")
    }
}

impl TranspileableExpr for ExtractExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let object_type = self.object.type_of(context).map_err(|e| {
            e.context("...while determining type of object expression for extraction")
        })?;
        let index_type = self.index.type_of(context).map_err(|e| {
            e.context("...while determining type of index expression for extraction")
        })?;

        let result_type = rosy_lib::BinaryOp::Extract
            .return_type(&object_type, &index_type)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Cannot extract from type '{}' using index of type '{}'!",
                    object_type,
                    index_type
                )
            })?;

        Ok(result_type)
    }
    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> ExprFunctionCallResult {
        if let Err(e) = resolver.discover_expr_function_calls(&self.object, ctx) {
            return ExprFunctionCallResult::HasFunctionCalls { result: Err(e) };
        }
        ExprFunctionCallResult::HasFunctionCalls {
            result: resolver.discover_expr_function_calls(&self.index, ctx),
        }
    }
    fn build_expr_recipe(
        &self,
        resolver: &TypeResolver,
        ctx: &ScopeContext,
        deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        let left = resolver.build_expr_recipe(&self.object, ctx, deps);
        let right = resolver.build_expr_recipe(&self.index, ctx, deps);
        ExprRecipe::BinaryOp {
            op: BinaryOpKind::Extract,
            left: Box::new(left),
            right: Box::new(right),
        }
    }
}
impl Transpile for ExtractExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // First, ensure the types are compatible
        let _ = self
            .type_of(context)
            .map_err(|e| vec![e.context("...while verifying types of extraction expression")])?;

        // Then, transpile both sides and combine
        let mut errors = Vec::new();
        let mut requested_variables = BTreeSet::new();

        // Transpile object
        let object_output = match self.object.transpile(context) {
            Ok(output) => {
                requested_variables.extend(output.requested_variables.iter().cloned());
                output
            }
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling object of extraction"));
                }
                TranspilationOutput::default()
            }
        };

        // Transpile index
        let index_output = match self.index.transpile(context) {
            Ok(output) => {
                requested_variables.extend(output.requested_variables.iter().cloned());
                output
            }
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling index of extraction"));
                }
                TranspilationOutput::default()
            }
        };

        let serialization = format!(
            "RosyExtract::rosy_extract({}, {}).context(\"...while trying to extract an element\")?",
            object_output.as_ref(),
            index_output.as_ref()
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
