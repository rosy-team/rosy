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
    compiler::name_map::NameMap,
    program::statements::SourceLocation,
    resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot},
    syntax_config,
};
use anyhow::{Error, Result};
use rosy_lib::{RosyBaseType, RosyType};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

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
    /// Optimization: if this expression is `target & expr` (bare or indexed),
    /// append in-place onto `dest` instead of clone + concat + assign.
    fn try_inplace_append(
        &self,
        _target_var: &str,
        _target_indices: &[String],
        _dest: &str,
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
    /// COSY `VARIABLE name <mem> <dims...>` array rank.
    /// `Some(0)` is a scalar cell (the memory size is not a dimension).
    /// `None` is an argument or loop index, which COSY will index if the
    /// caller passed an array.
    pub declared_dims: Option<usize>,
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
    pub requested_types: HashMap<String, RosyType>,
    /// Parent local that shadows a different outer cell. Threaded as `__loc_<name>`.
    pub loc_shadows: BTreeMap<String, RosyType>,
    /// Capture named the visible same-type shadow, not the outer cell.
    /// Set for a procedure or function nested under that shadow.
    pub inner_shadows: BTreeSet<String>,
}
#[derive(Debug, Clone)]
pub struct TranspilationInputProcedureContext {
    pub args: Vec<VariableData>,
    pub requested_variables: BTreeSet<String>,
    pub requested_types: HashMap<String, RosyType>,
    /// Parent local that shadows a different outer cell. Threaded as `__loc_<name>`.
    pub loc_shadows: BTreeMap<String, RosyType>,
    /// Capture named the visible same-type shadow, not the outer cell.
    pub inner_shadows: BTreeSet<String>,
    /// Defined in a `.fox` file, so the rust fn is `__proc_<name>`.
    pub cosy_syntax: bool,
}
#[derive(Default, Clone)]
pub struct TranspilationInputContext {
    pub variables: NameMap<ScopedVariableData>,
    pub functions: NameMap<TranspilationInputFunctionContext>,
    pub procedures: NameMap<TranspilationInputProcedureContext>,
    pub in_loop: bool,
    /// True inside IF/ELSEIF/ELSE rust blocks. Locals may be redeclared
    /// (shadowed) without treating the parent binding as a captured Higher ref.
    pub in_block: bool,
    /// Higher bindings hidden by a local of the same name. RK's step `H`
    /// shadows the global curvature `H`; ODE/POTFLD still need the outer one.
    pub outer_bindings: NameMap<ScopedVariableData>,
    /// Set while emitting COSY `PROCEDURE RK`, whose local step `H` must not
    /// hide the global curvature `H` captured by ODE/POTFLD.
    pub split_rk_h: bool,
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

    pub fn loc_ident(name: &str) -> String {
        format!("__loc_{name}")
    }

    /// Local/arg that shadows an outer binding. Keep both: callee captures
    /// still receive the outer cell (COSY WRAP A / USEA writing global A).
    /// COSY RK's step `H` vs global curvature `H` is the same pattern even when
    /// both are `f64`.
    pub fn uses_loc_ident(&self, name: &str) -> bool {
        let is_step_h = if syntax_config::is_cosy_syntax() {
            name.eq_ignore_ascii_case("H")
        } else {
            name == "H"
        };
        if self.split_rk_h
            && is_step_h
            && self
                .variables
                .get(name)
                .is_some_and(|v| v.scope == VariableScope::Local)
        {
            return true;
        }
        let Some(cur) = self.variables.get(name) else {
            return false;
        };
        let Some(outer) = self.outer_bindings.get(name) else {
            return false;
        };
        if matches!(cur.scope, VariableScope::Local | VariableScope::Arg) {
            return true;
        }
        // Nested procedure: the parent's local still shadows a different
        // outer cell. Keep the outer name (`A`) and the shadow (`__loc_A`).
        cur.scope == VariableScope::Higher
            && cur.data.r#type.as_rust_type() != outer.data.r#type.as_rust_type()
    }

    /// Shadow living in a parent, so this fn must take it as `__loc_<name>`.
    pub fn higher_shadow_type(&self, name: &str) -> Option<RosyType> {
        if !self.uses_loc_ident(name) {
            return None;
        }
        let cur = self.variables.get(name)?;
        if cur.scope != VariableScope::Higher {
            return None;
        }
        Some(cur.data.r#type)
    }

    /// COSY RK names its step `H`, same as global curvature. Keep both.
    /// Typed rosy locals that shadow a fox cell of the same name also split.
    pub fn rust_ident(&self, name: &str) -> String {
        // Emit the declaration's spelling. A `.fox` use may differ in case.
        let declared = self
            .variables
            .get(name)
            .map(|v| v.data.name.as_str())
            .unwrap_or(name);
        if self.uses_loc_ident(name) {
            Self::loc_ident(declared)
        } else {
            declared.to_string()
        }
    }

    /// Parameter name for a captured cell. The body emits `rust_ident`
    /// (the first declaration's spelling). A loc-split outer cell keeps
    /// that bare spelling; the shadow is `shadow_param_name`.
    pub fn capture_param_name(&self, name: &str) -> String {
        if self.uses_loc_ident(name) {
            self.outer_bindings
                .get(name)
                .map(|o| o.data.name.clone())
                .unwrap_or_else(|| name.to_string())
        } else {
            self.rust_ident(name)
        }
    }

    /// `__loc_` parameter. The body spells this from the declaration,
    /// not from the use site that landed in `requested_variables`.
    pub fn shadow_param_name(&self, name: &str) -> String {
        let declared = self
            .variables
            .get(name)
            .map(|v| v.data.name.clone())
            .unwrap_or_else(|| name.to_string());
        Self::loc_ident(&declared)
    }

    /// Callee closed over `wanted`. A same rust type means the outer cell
    /// (RK's step `H` must not replace global curvature `H`). A callee
    /// nested under the shadow lists the name in `inner_shadows` and is
    /// excluded by [`Self::pass_outer_cell`].
    pub fn capture_uses_outer(&self, name: &str, wanted: &RosyType) -> bool {
        if !self.uses_loc_ident(name) {
            return false;
        }
        let Some(outer) = self.outer_bindings.get(name) else {
            return false;
        };
        outer.data.r#type.as_rust_type() == wanted.as_rust_type()
    }

    /// Visible binding is a same-type shadow of an outer cell. A nested
    /// procedure that mentions the name closed over this cell.
    pub fn closes_over_visible_shadow(&self, name: &str) -> bool {
        if self.uses_loc_ident(name) {
            return false;
        }
        let Some(cur) = self.variables.get(name) else {
            return false;
        };
        cur.scope == VariableScope::Higher && self.outer_bindings.contains_key(name)
    }

    /// Pass the outer cell rather than the `__loc_` shadow.
    pub fn pass_outer_cell(
        &self,
        name: &str,
        wanted: &RosyType,
        inner_shadows: &BTreeSet<String>,
    ) -> bool {
        if inner_shadows.contains(name) {
            return false;
        }
        self.capture_uses_outer(name, wanted) || (self.split_rk_h && name == "H")
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
            // An array read that is only borrowed does not need its own copy.
            ValueKind::Owned if is_single_call(&self.serialization, "rosy_get(") => {
                format!(
                    "&*rosy_get_ref({}",
                    &self.serialization["rosy_get(".len()..]
                )
            }
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

/// `expr` is exactly one call `name...(...)`, with nothing after its `)`.
fn is_single_call(expr: &str, name: &str) -> bool {
    if !expr.starts_with(name) {
        return false;
    }
    let mut depth = 0i32;
    let mut in_str = false;
    let mut prev = '\0';
    for (i, c) in expr.char_indices().skip(name.len() - 1) {
        if in_str {
            if c == '"' && prev != '\\' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
        } else if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                return i == expr.len() - 1;
            }
        }
        prev = c;
    }
    false
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
pub fn emit_pass_as(
    name: &str,
    provided: &RosyType,
    expected: &RosyType,
    scope: VariableScope,
) -> (Vec<String>, String, Vec<String>) {
    let mut prelude = Vec::new();
    let mut writeback = Vec::new();
    let pass = match scope {
        VariableScope::Local => format!("&mut {name}"),
        VariableScope::Arg | VariableScope::Higher => name.to_string(),
    };
    if provided.as_rust_type() == expected.as_rust_type() {
        return (prelude, pass, writeback);
    }
    if expected.is_any() && expected.dimensions == 0 {
        let tmp = format!("__rosy_cap_{name}");
        prelude.push(format!(
            "let mut {tmp} = RosyValue::from(({name}).clone());"
        ));
        let lhs = match scope {
            VariableScope::Local => name.to_string(),
            VariableScope::Arg | VariableScope::Higher => format!("*{name}"),
        };
        writeback.push(format!(
            "{lhs} = {};",
            emit_unwrap_rosy_value(tmp.clone(), provided)
        ));
        return (prelude, format!("&mut {tmp}"), writeback);
    }
    if provided.is_any() {
        let tmp = format!("__rosy_cap_{name}");
        let src = if provided.dimensions > 0 {
            format!("RosyValue::from(({name}).clone())")
        } else {
            format!("({name}).clone()")
        };
        prelude.push(format!(
            "let mut {tmp} = {};",
            emit_unwrap_rosy_value(src, expected)
        ));
        return (prelude, format!("&mut {tmp}"), writeback);
    }
    let exp_rt = expected.as_rust_type();
    let got_rt = provided.as_rust_type();
    if exp_rt == "Vec<f64>" && got_rt == "f64" {
        let tmp = format!("__rosy_cap_{name}");
        prelude.push(format!("let mut {tmp} = vec![({name}).clone()];"));
        return (prelude, format!("&mut {tmp}"), writeback);
    }
    if exp_rt == "Vec<Vec<f64>>" && got_rt == "f64" {
        let tmp = format!("__rosy_cap_{name}");
        prelude.push(format!("let mut {tmp} = vec![vec![({name}).clone()]];"));
        return (prelude, format!("&mut {tmp}"), writeback);
    }
    if exp_rt == "Vec<Vec<f64>>" && got_rt == "Vec<f64>" {
        let tmp = format!("__rosy_cap_{name}");
        prelude.push(format!("let mut {tmp} = vec![({name}).clone()];"));
        return (prelude, format!("&mut {tmp}"), writeback);
    }
    if exp_rt == "f64" && got_rt == "Vec<f64>" {
        let tmp = format!("__rosy_cap_{name}");
        prelude.push(format!(
            "let mut {tmp} = ({name}).first().copied().unwrap_or(0.0);"
        ));
        return (prelude, format!("&mut {tmp}"), writeback);
    }
    if expected.is_any() && expected.dimensions > 0 && provided.dimensions == 0 {
        let tmp = format!("__rosy_cap_{name}");
        let mut cell = format!("RosyValue::from(({name}).clone())");
        for _ in 0..expected.dimensions {
            cell = format!("vec![{cell}]");
        }
        prelude.push(format!("let mut {tmp} = {cell};"));
        return (prelude, format!("&mut {tmp}"), writeback);
    }
    (prelude, pass, writeback)
}

pub fn needs_box_as_any(provided: &RosyType, expected: &RosyType) -> bool {
    expected.is_any() && expected.dimensions == 0 && (provided.dimensions > 0 || !provided.is_any())
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
    if ty.dimensions > 0 && ty.base_type == RosyBaseType::CD {
        return format!("({expr}).expect_arr()?.into_iter().map(|x| x.as_cd()).collect()");
    }
    if ty.dimensions > 0 && ty.base_type == RosyBaseType::DA {
        return format!(
            "({expr}).expect_arr()?.into_iter().map(|x| x.expect_da().unwrap_or_else(|_| DA::zero())).collect()"
        );
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
