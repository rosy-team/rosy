//! # Derivation Operator (`%`)
//!
//! Computes partial derivatives or anti-derivatives of DA/CD Taylor series.
//!
//! ## Syntax
//!
//! ```text
//! da_expr % n       { partial derivative w.r.t. variable n (n > 0) }
//! da_expr % (-n)    { anti-derivative (integral) w.r.t. variable n }
//! ```
//!
//! ## Supported Types
//!
//! | Object | Result |
//! |--------|--------|
//! | DA | DA |
//! | CD | CD |
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/expressions/operators/collection/derive.rosy"))]
//! ```

use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use rosy_lib::BinaryOp;
use crate::transpile::{
    ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
    TranspileableExpr, ValueKind,
};
use anyhow::{Context as AnyhowContext, Error, Result};
use rosy_lib::RosyType;
use std::collections::{BTreeSet, HashSet};

/// DA%n = partial derivative w.r.t. variable n (positive n)
/// DA%(-n) = anti-derivative (integral) w.r.t. variable n (negative n)
#[derive(Debug)]
pub struct DeriveExpr {
    pub object: Box<Expr>,
    pub index: Box<Expr>,
}

impl Transpile for DeriveExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let object_output = self.object.transpile(context)?;
        let index_output = self.index.transpile(context)?;

        let mut requested_variables = BTreeSet::new();
        requested_variables.extend(object_output.requested_variables.iter().cloned());
        requested_variables.extend(index_output.requested_variables.iter().cloned());

        // Generate code that checks the sign of the index at runtime:
        // positive => derivative, negative => antiderivative
        let serialization = format!(
            "RosyDerive::rosy_derive({}, ({}).clone() as i64)?",
            object_output.as_ref(),
            index_output.as_value()
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
impl TranspileableExpr for DeriveExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let object_type = self
            .object
            .type_of(context)
            .context("Failed to determine type of object in % (derive) expression")?;

        match object_type {
            t if t == RosyType::DA() => Ok(RosyType::DA()),
            t if t == RosyType::CD() => Ok(RosyType::CD()),
            _ => anyhow::bail!(
                "Derivation operator % not supported for type: {:?}. Only DA and CD are supported.",
                object_type
            ),
        }
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
            op: BinaryOp::Derive,
            left: Box::new(left),
            right: Box::new(right),
        }
    }
}
