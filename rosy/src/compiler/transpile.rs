//! # Transpilation Engine
//!
//! Core traits and context types for converting the Rosy AST into Rust source code.
//!
//! ## Key Traits
//!
//! | Trait | Purpose |
//! |-------|---------|
//! | [`Transpile`] | Converts an AST node to a Rust code string |
//! | [`TranspileableStatement`] | Represents a statement node that can be transpiled |
//! | [`TranspileableExpr`] | Represents an expression node that can be transpiled |
//!
//! ## Context
//!
//! [`TranspilationInputContext`] tracks variable scope, function/procedure
//! signatures, and closure-captured variables during transpilation.
//!
//! ## Error Handling
//!
//! Transpilation returns `Result<TranspilationOutput, Vec<Error>>` to
//! accumulate multiple errors before failing. Use `.context()` to add
//! breadcrumbs for error diagnostics.

use crate::{
    program::statements::SourceLocation,
    resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot},
};
use anyhow::{Error, Result};
use rosy_lib::{RosyBaseType, RosyType};
use std::collections::{BTreeSet, HashMap, HashSet};

pub trait TranspileableStatement: Transpile {
    fn register_typeslot_declaration(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> Option<Result<()>> {
        None
    }
    fn wire_inference_edges(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> Option<Result<()>> {
        None
    }
    fn hydrate_resolved_types(
        &mut self,
        _resolver: &TypeResolver,
        _current_scope: &[String],
    ) -> Option<Result<()>> {
        None
    }
}
pub trait TranspileableExpr: Transpile {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType>;
    fn discover_expr_function_calls(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &ScopeContext,
    ) -> Option<Result<()>> {
        None
    }
    fn build_expr_recipe(
        &self,
        resolver: &TypeResolver,
        ctx: &ScopeContext,
        deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe;
    /// Returns Some(name) if this expression is a bare variable reference (no indices).
    /// Used by optimizations that detect self-referential patterns like `X := X & val`.
    fn as_bare_variable_name(&self) -> Option<&str> {
        None
    }
    /// Optimization: if this expression is `target_var & expr`, return code
    /// that appends in-place (push/extend) instead of clone + concat + assign.
    /// Returns None if the optimization doesn't apply (default).
    fn try_inplace_append(
        &self,
        _target_var: &str,
        _context: &mut TranspilationInputContext,
    ) -> Option<Result<TranspilationOutput, Vec<Error>>> {
        None
    }
}
pub trait Transpile: std::fmt::Debug {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum VariableScope {
    Local,
    Arg,
    Higher,
}
#[derive(Debug, Clone)]
pub struct VariableData {
    pub name: String,
    pub r#type: RosyType,
}
#[derive(Debug, Clone)]
pub struct ScopedVariableData {
    pub scope: VariableScope,
    pub data: VariableData,
}
#[derive(Debug, Clone)]
pub struct TranspilationInputFunctionContext {
    pub return_type: RosyType,
    pub args: Vec<VariableData>,
    pub requested_variables: BTreeSet<String>,
}
#[derive(Debug, Clone)]
pub struct TranspilationInputProcedureContext {
    pub args: Vec<VariableData>,
    pub requested_variables: BTreeSet<String>,
}
#[derive(Default, Clone)]
pub struct TranspilationInputContext {
    pub variables: HashMap<String, ScopedVariableData>,
    pub functions: HashMap<String, TranspilationInputFunctionContext>,
    pub procedures: HashMap<String, TranspilationInputProcedureContext>,
    pub in_loop: bool,
}

impl TranspilationInputContext {
    /// Find a case-insensitive match for `name` among the given candidates.
    /// Returns a hint string like " (did you mean 'FOO'? Rosy is case-sensitive)" or empty.
    pub fn case_hint<'a>(name: &str, mut candidates: impl Iterator<Item = &'a String>) -> String {
        let name_upper = name.to_uppercase();
        candidates
            .find(|c| c.to_uppercase() == name_upper && *c != name)
            .map(|c| format!(" (did you mean '{}'? Rosy is case-sensitive)", c))
            .unwrap_or_default()
    }

    /// Hint for an undeclared variable name.
    pub fn variable_hint(&self, name: &str) -> String {
        Self::case_hint(name, self.variables.keys())
    }

    /// Hint for an undeclared procedure name.
    pub fn procedure_hint(&self, name: &str) -> String {
        Self::case_hint(name, self.procedures.keys())
    }

    /// Hint for an undeclared function name.
    pub fn function_hint(&self, name: &str) -> String {
        Self::case_hint(name, self.functions.keys())
    }
}

/// Whether an expression produces an owned value or a reference.
///
/// This drives how consumers wrap the expression:
/// - Owned + needs ref → `&expr`
/// - Ref + needs owned (Copy) → `expr` (auto-deref)
/// - Ref + needs owned (non-Copy) → `expr.clone()`
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ValueKind {
    /// A fresh value: literals, operator results, function returns.
    /// Can be moved into assignment without cloning.
    #[default]
    Owned,
    /// A reference to an existing variable (`&X` or `X` where X: &T).
    /// Must be cloned to own for non-Copy types.
    Ref,
}

#[derive(Default)]
pub struct TranspilationOutput {
    pub serialization: String,
    pub requested_variables: BTreeSet<String>,
    pub value_kind: ValueKind,
}

impl TranspilationOutput {
    /// Get this expression as an owned value for assignment or value context.
    /// - Owned → expr (move/copy)
    /// - Ref(&X) + Copy → X (strip & to get bare name)
    /// - Ref(X) + Copy → (*X) (deref &mut T)
    /// - Ref(&X) + non-Copy → X.clone()
    /// - Ref(X) + non-Copy → (*X).clone()
    pub fn as_owned(&self, ty: &RosyType) -> String {
        match self.value_kind {
            ValueKind::Owned => self.serialization.clone(),
            ValueKind::Ref => {
                if let Some(inner) = self.serialization.strip_prefix('&') {
                    if ty.is_copy() {
                        inner.to_string()
                    } else {
                        format!("{inner}.clone()")
                    }
                } else if ty.is_copy() {
                    format!("(*{})", self.serialization)
                } else {
                    format!("(*{}).clone()", self.serialization)
                }
            }
        }
    }

    /// Get this expression as a shared reference for trait method arguments.
    /// - Owned → &expr
    /// - Ref(&X) → &X (already a shared reference)
    /// - Ref(X) → &*X (deref &mut T to get &T)
    pub fn as_ref(&self) -> String {
        match self.value_kind {
            ValueKind::Owned => format!("&{}", self.serialization),
            ValueKind::Ref => {
                if self.serialization.starts_with('&') {
                    self.serialization.clone()
                } else {
                    format!("&*{}", self.serialization)
                }
            }
        }
    }

    /// Get this expression as a mutable reference for function/procedure arguments.
    /// - Owned → &mut expr
    /// - Ref(&X) → &mut X (strip & and add &mut)
    /// - Ref(X) → &mut *X (deref &mut T to get &mut T)
    pub fn as_mut_ref(&self) -> String {
        match self.value_kind {
            ValueKind::Owned => format!("&mut {}", self.serialization),
            ValueKind::Ref => {
                if let Some(inner) = self.serialization.strip_prefix('&') {
                    format!("&mut {}", inner)
                } else {
                    format!("&mut *{}", self.serialization)
                }
            }
        }
    }

    /// Get this expression as a plain value (for arithmetic, conditions, casts).
    /// Refs clone so `ANY` / `RosyValue` args do not move out of `&mut`.
    /// - Owned → expr
    /// - Ref(&X) → X.clone()
    /// - Ref(X) → (*X).clone()
    pub fn as_value(&self) -> String {
        match self.value_kind {
            ValueKind::Owned => self.serialization.clone(),
            ValueKind::Ref => {
                if let Some(inner) = self.serialization.strip_prefix('&') {
                    format!("{inner}.clone()")
                } else {
                    format!("(*{}).clone()", self.serialization)
                }
            }
        }
    }

    /// Numeric context: `f64`, unwrapping `RosyValue` when `ty` is ANY.
    pub fn as_re(&self, ty: &RosyType) -> String {
        if ty.is_any() {
            format!("rosy_as_f64(&({}))", self.as_value())
        } else {
            self.as_value()
        }
    }
}

pub fn types_compatible(provided: &RosyType, expected: &RosyType) -> bool {
    provided == expected || provided.is_any() || expected.is_any()
}

/// `&RosyValue` for dyn dispatch, wrapping a concrete value when needed.
pub fn emit_as_rosy_value_ref(out: &TranspilationOutput, ty: &RosyType) -> String {
    if ty.is_any() {
        out.as_ref()
    } else {
        format!("&RosyValue::from({})", out.as_ref())
    }
}

/// Unwrap a `RosyValue` expression to a concrete rust type.
pub fn needs_box_as_any(provided: &RosyType, expected: &RosyType) -> bool {
    expected.is_any()
        && expected.dimensions == 0
        && (provided.dimensions > 0 || !provided.is_any())
}

pub fn emit_unwrap_rosy_value(expr: String, ty: &RosyType) -> String {
    if ty.is_any() && ty.dimensions >= 2 {
        return format!("({expr}).expect_arr2()?");
    }
    if ty.is_any() && ty.dimensions == 1 {
        return format!("({expr}).expect_arr()?");
    }
    if ty.dimensions >= 2 && ty.base_type == RosyBaseType::RE {
        return format!("({expr}).expect_re2()?");
    }
    if ty.dimensions > 0 && ty.base_type == RosyBaseType::RE {
        return format!("({expr}).expect_ve()?");
    }
    match ty.base_type {
        RosyBaseType::ANY => expr,
        RosyBaseType::RE => format!("({expr}).expect_re()?"),
        RosyBaseType::ST => format!("({expr}).expect_st()?"),
        RosyBaseType::LO => format!("({expr}).expect_lo()?"),
        RosyBaseType::CM => format!("({expr}).expect_cm()?"),
        RosyBaseType::VE => format!("({expr}).expect_ve()?"),
        RosyBaseType::DA => format!("({expr}).expect_da()?"),
        RosyBaseType::CD => format!("({expr}).expect_cd()?"),
    }
}

// helper for indenting blocks
pub fn indent(st: String) -> String {
    st.lines()
        .map(|line| format!("\t{}", line))
        .collect::<Vec<String>>()
        .join("\n")
}
// helper for adding context to a vec of  errors
pub fn add_context_to_all(arr: Vec<Error>, context: String) -> Vec<Error> {
    arr.into_iter()
        .map(|err| err.context(context.clone()))
        .collect()
}
