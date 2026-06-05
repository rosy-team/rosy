//! # Concatenation Operator (`&`)
//!
//! Concatenates scalars and vectors into larger vectors, or strings together.
//!
//! ## Syntax
//!
//! ```text
//! expr & expr
//! ```
//!
//! ## Type Compatibility
//!
//! | Left | Right | Result | Comment |
//! |------|-------|--------|---------|
//! | RE | RE | VE | Concatenate two Reals to a Vector |
//! | RE | VE | VE | Prepend a Real to the left of a Vector |
//! | ST | ST | ST | Concatenate two Strings |
//! | VE | RE | VE | Append a Real to the right of a Vector |
//! | VE | VE | VE | Concatenate two Vectors |
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

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::rosy_lib::{RosyBaseType, RosyType};
use crate::transpile::{
    ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput,
    Transpile, TranspileableExpr, ValueKind, VariableScope,
};
use anyhow::{Context, Error, Result};
use std::collections::BTreeSet;
use std::collections::HashSet;

/// AST node for the concatenation operator (`&`).
#[derive(Debug)]
pub struct ConcatExpr {
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

impl FromRule for ConcatExpr {
    fn from_rule(_pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::bail!("ConcatExpr should be created by infix parser, not FromRule")
    }
}
impl TranspileableExpr for ConcatExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let left_type = self
            .left
            .type_of(context)
            .context("...while determining type of left side of concatenation")?;
        let right_type = self
            .right
            .type_of(context)
            .context("...while determining type of right side of concatenation")?;

        crate::rosy_lib::operators::concat::get_return_type(&left_type, &right_type)
            .ok_or_else(|| anyhow::anyhow!(
                "Cannot concatenate types '{}' and '{}' together!",
                left_type,
                right_type
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
        if let Err(e) = resolver.discover_expr_function_calls(&self.right, ctx) {
            return ExprFunctionCallResult::HasFunctionCalls { result: Err(e) };
        }
        ExprFunctionCallResult::HasFunctionCalls { result: Ok(()) }
    }
    fn build_expr_recipe(
        &self,
        resolver: &TypeResolver,
        ctx: &ScopeContext,
        deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        let left = resolver.build_expr_recipe(&self.left, ctx, deps);
        let right = resolver.build_expr_recipe(&self.right, ctx, deps);
        ExprRecipe::Concat(Box::new(left), Box::new(right))
    }
    fn try_inplace_append(
        &self,
        target_var: &str,
        context: &mut TranspilationInputContext,
    ) -> Option<Result<TranspilationOutput, Vec<Error>>> {
        // Check if left operand is a bare variable matching the assignment target
        let left_name = self.left.inner.as_bare_variable_name()?;
        if left_name != target_var {
            return None;
        }

        // Determine the concat pattern from types
        let left_type = self.left.type_of(context).ok()?;
        let right_type = self.right.type_of(context).ok()?;

        // Determine the append operation
        let is_push = match (left_type.base_type, left_type.dimensions, right_type.base_type, right_type.dimensions) {
            // VE & RE → push (f64 is Copy, no clone needed)
            (RosyBaseType::VE, 0, RosyBaseType::RE, 0) => true,
            // VE & VE → extend
            (RosyBaseType::VE, 0, RosyBaseType::VE, 0) => false,
            // (DA N) & DA → push (DA needs clone)
            (RosyBaseType::DA, d, RosyBaseType::DA, 0) if d > 0 => true,
            // (DA N) & (DA N) → extend
            (RosyBaseType::DA, d1, RosyBaseType::DA, d2) if d1 > 0 && d2 > 0 => false,
            // (CD N) & CD → push
            (RosyBaseType::CD, d, RosyBaseType::CD, 0) if d > 0 => true,
            // (CD N) & (CD N) → extend
            (RosyBaseType::CD, d1, RosyBaseType::CD, d2) if d1 > 0 && d2 > 0 => false,
            // Not an append pattern we can optimize
            _ => return None,
        };

        // Transpile the right operand
        let right_output = match self.right.transpile(context) {
            Ok(out) => out,
            Err(e) => return Some(Err(e)),
        };

        let mut requested_variables = BTreeSet::new();
        requested_variables.extend(right_output.requested_variables.iter().cloned());

        // Resolve target_var's scope so generated `target.push(...)` /
        // `target.extend_from_slice(...)` autoderefs correctly. Higher-scope
        // (auto-captured global) and Arg-scope (procedure parameter) vars
        // arrive as `&mut Vec<_>` / `&mut Vec<DA>` etc., so the method call
        // needs a `(*target).` deref. Local vars resolve to bare names.
        let target_deref = match context
            .variables
            .get(target_var)
            .map(|v| &v.scope)
        {
            Some(VariableScope::Local) => "",
            Some(VariableScope::Arg) => "*",
            Some(VariableScope::Higher) => {
                requested_variables.insert(target_var.to_string());
                "*"
            }
            None => "",
        };

        let needs_clone = matches!(right_type.base_type, RosyBaseType::DA | RosyBaseType::CD);

        let code = if is_push {
            let val = right_output.as_value();
            if needs_clone {
                format!("{{ let __v = {val}.clone(); ({target_deref}{target_var}).push(__v); }}")
            } else {
                format!("{{ let __v = {val}; ({target_deref}{target_var}).push(__v); }}")
            }
        } else {
            let val_ref = right_output.as_ref();
            format!("{{ let __v: Vec<_> = ({val_ref}).to_vec(); ({target_deref}{target_var}).extend_from_slice(&__v); }}")
        };

        Some(Ok(TranspilationOutput {
            serialization: code,
            requested_variables,
            ..Default::default()
        }))
    }
}
impl Transpile for ConcatExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Type check
        let _ = self
            .type_of(context)
            .map_err(|e| vec![e.context("...while verifying types of concatenation expression")])?;

        let left_output = self.left.transpile(context)
            .map_err(|errs| errs.into_iter()
                .map(|e| e.context("...while transpiling left side of concatenation"))
                .collect::<Vec<_>>())?;
        let right_output = self.right.transpile(context)
            .map_err(|errs| errs.into_iter()
                .map(|e| e.context("...while transpiling right side of concatenation"))
                .collect::<Vec<_>>())?;

        let mut requested_variables = BTreeSet::new();
        requested_variables.extend(left_output.requested_variables.iter().cloned());
        requested_variables.extend(right_output.requested_variables.iter().cloned());

        Ok(TranspilationOutput {
            serialization: format!(
                "RosyConcat::rosy_concat({}, {})?",
                left_output.as_ref(),
                right_output.as_ref()
            ),
            requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
