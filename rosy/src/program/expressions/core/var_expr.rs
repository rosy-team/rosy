//! # Variable Expressions & Function Call Disambiguation
//!
//! A `variable_identifier` in the parse tree can represent either a plain
//! variable access (with optional indexing) or a user-defined function call.
//!
//! At transpile time, [`VarExpr::classify`] applies a decision tree to
//! determine which interpretation is correct based on scope context.
//!
//! ## Decision Tree
//!
//! | Paren Groups | Args per Group | Bracket Indices | Result |
//! |-------------|----------------|-----------------|--------|
//! | 0 | — | any | Variable |
//! | 1 | multiple | — | Context-dependent: function call vs COSY-style multi-dim index |
//! | 1 | 1 | — | Context-dependent (see below) |
//! | ≥2 | 1 each | — | Multi-dim index (Variable) |
//!
//! ### Single-arg disambiguation (1 paren group, 1 arg)
//!
//! | Variable? | Function? | Variable dims | Result |
//! |-----------|-----------|---------------|--------|
//! | yes | no | — | Variable (index) |
//! | no | yes | — | Function call |
//! | yes | yes | >0 (array) | Variable (index) |
//! | yes | yes | 0 (scalar) | Function call (recursion) |
//! | no | no | — | Error |
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/expressions/core/var_expr.rosy"))]
//! ```

use super::variable_identifier::VariableIdentifier;
use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::transpile::TranspileableExpr;
use crate::transpile::{
    TranspilationInputContext, TranspilationOutput, Transpile, ValueKind, VariableScope,
};
use anyhow::{Context, Error, Result, anyhow};
use rosy_lib::RosyType;
use std::collections::BTreeSet;
use std::collections::HashSet;

use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};

/// Find a case-insensitive match among known names and return a hint string.
fn find_case_similar<'a>(name: &str, candidates: impl Iterator<Item = &'a String>) -> String {
    let name_upper = name.to_uppercase();
    let matches: Vec<&String> = candidates
        .filter(|c| c.to_uppercase() == name_upper && *c != name)
        .collect();
    if matches.is_empty() {
        String::new()
    } else {
        format!(" (did you mean '{}'? Rosy is case-sensitive)", matches[0])
    }
}

/// What a `variable_identifier` AST node actually represents,
/// determined at transpile time via the decision tree.
#[derive(Debug)]
pub enum VarExprKind {
    /// Plain variable or variable with indexing: `X`, `X(I)`, `X(I)(J)`, `X[I,J]`
    Variable,
    /// Function call: `FUNC(a, b)` or `FUNC(x)` when FUNC is a known function
    FunctionCall,
}

#[derive(Debug)]
pub struct VarExpr {
    pub identifier: VariableIdentifier,
}

impl VarExpr {
    /// Apply the disambiguation decision tree:
    ///
    /// - Multiple paren groups → multi-dimensional indexing (Variable)
    /// - One paren group with multiple args → function call
    /// - One paren group with one arg → check context: function wins if only function matches,
    ///   variable wins if only variable matches
    /// - No paren groups → variable
    /// - Any invalid combo → error
    pub fn classify(&self, context: &TranspilationInputContext) -> Result<VarExprKind, Vec<Error>> {
        let ident = &self.identifier;
        let num_groups = ident.paren_groups.len();
        let has_brackets = !ident.bracket_indices.is_empty();

        match num_groups {
            0 => {
                // No parens — always a variable (possibly with bracket indices)
                Ok(VarExprKind::Variable)
            }
            1 => {
                let num_args = ident.paren_groups[0].len();
                if num_args > 1 {
                    // Multiple args in one paren group — could be either a
                    // function call `FUNC(a, b)` or COSY-style multi-dim
                    // indexing `X(I, J)`. Disambiguate via the symbol table.
                    let is_var = context.variables.contains_key(&ident.name);
                    let is_func = context.functions.contains_key(&ident.name);

                    let func_accepts = is_func
                        && context.functions.get(&ident.name).unwrap().args.len() == num_args;
                    let var_accepts = is_var && {
                        let v = context.variables.get(&ident.name).unwrap();
                        // VE only accepts a single index, so it can never satisfy
                        // a multi-arg group; fall through to function-call routing.
                        v.data.r#type.base_type != rosy_lib::RosyBaseType::VE
                            && num_args <= v.data.r#type.dimensions
                    };

                    let route_as_call = match (func_accepts, var_accepts) {
                        (true, _) => true,                    // function arity matches — prefer call
                        (false, true) => false, // only variable fits — multi-dim index
                        (false, false) => is_func || !is_var, // surface the more informative error downstream
                    };

                    if route_as_call {
                        if has_brackets {
                            return Err(vec![anyhow::anyhow!(
                                "'{}': function call with bracket indexing is not valid",
                                ident.name
                            )]);
                        }
                        Ok(VarExprKind::FunctionCall)
                    } else {
                        Ok(VarExprKind::Variable)
                    }
                } else {
                    // Single paren group, single arg → check context
                    let is_var = context.variables.contains_key(&ident.name);
                    let is_func = context.functions.contains_key(&ident.name);

                    match (is_var, is_func) {
                        (true, false) => Ok(VarExprKind::Variable),
                        (false, true) => {
                            if has_brackets {
                                return Err(vec![anyhow::anyhow!(
                                    "'{}': function call with bracket indexing is not valid",
                                    ident.name
                                )]);
                            }
                            Ok(VarExprKind::FunctionCall)
                        }
                        (true, true) => {
                            // Both exist — disambiguate by checking variable dimensions.
                            // A scalar variable (0 dimensions) cannot be indexed, so
                            // parentheses must be a function call (e.g. recursion where
                            // the function name doubles as the return variable).
                            let var_data = context.variables.get(&ident.name).unwrap();
                            if var_data.data.r#type.dimensions > 0 {
                                // Variable is an array — prefer indexing
                                Ok(VarExprKind::Variable)
                            } else {
                                // Variable is a scalar — can't index, must be a function call
                                if has_brackets {
                                    return Err(vec![anyhow::anyhow!(
                                        "'{}': function call with bracket indexing is not valid",
                                        ident.name
                                    )]);
                                }
                                Ok(VarExprKind::FunctionCall)
                            }
                        }
                        (false, false) => Err(vec![anyhow::anyhow!(
                            "'{}' is neither a defined variable nor a defined function in this scope!",
                            ident.name
                        )]),
                    }
                }
            }
            _ => {
                // Multiple paren groups → multi-dimensional indexing.
                // Any mix of group sizes is allowed (e.g. `X(1, 2)(3)` is the
                // same as `X(1)(2)(3)`); total index count is validated against
                // the variable's dimensions during type_of().
                Ok(VarExprKind::Variable)
            }
        }
    }
}

impl FromRule for VarExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::variable_identifier,
            "Expected variable_identifier rule, got {:?}",
            pair.as_rule()
        );
        let identifier = VariableIdentifier::from_rule(pair)
            .context("Failed to build variable identifier!")?
            .ok_or_else(|| anyhow::anyhow!("Expected variable identifier"))?;
        Ok(Some(VarExpr { identifier }))
    }
}
impl TranspileableExpr for VarExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        match self.classify(context).map_err(|errs| {
            errs.into_iter()
                .next()
                .unwrap_or_else(|| anyhow::anyhow!("Unknown classification error"))
        })? {
            VarExprKind::FunctionCall => {
                let func_ctx = context
                    .functions
                    .get(&self.identifier.name)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Function '{}' not found{}",
                            self.identifier.name,
                            TranspilationInputContext::case_hint(
                                &self.identifier.name,
                                context.functions.keys()
                            )
                        )
                    })?;
                Ok(func_ctx.return_type)
            }
            VarExprKind::Variable => self.identifier.type_of(context).context(format!(
                "...while determining type of variable identifier '{}'",
                self.identifier.name
            )),
        }
    }
    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> Option<Result<()>> {
        let ident = &self.identifier;
        let num_groups = ident.paren_groups.len();
        let is_var = ctx.variables.contains_key(&ident.name);
        let is_func = ctx.functions.contains_key(&ident.name);

        let is_function_call = match num_groups {
            0 => false,
            1 => {
                let num_args = ident.paren_groups[0].len();
                if num_args > 1 {
                    // Mirror classify(): prefer function only when its arity
                    // matches; otherwise treat as multi-dim variable indexing
                    // when the variable can absorb that many indices.
                    let func_accepts = is_func
                        && ctx
                            .functions
                            .get(&ident.name)
                            .map(|(_, args)| args.len() == num_args)
                            .unwrap_or(false);
                    if func_accepts {
                        true
                    } else if is_var {
                        false
                    } else {
                        is_func
                    }
                } else {
                    !is_var && is_func
                }
            }
            _ => false,
        };

        if is_function_call {
            // Recursively discover function calls in each argument expression
            for arg in &ident.paren_groups[0] {
                if let Err(e) = resolver.discover_expr_function_calls(arg, ctx) {
                    return Some(Err(e));
                }
            }
            // Wire up call-site argument type dependencies
            Some(resolver.discover_call_site_deps(&ident.name, &ident.paren_groups[0], true, ctx))
        } else {
            // Variable access — recurse into any index expressions
            for group in &ident.paren_groups {
                for expr in group {
                    if let Err(e) = resolver.discover_expr_function_calls(expr, ctx) {
                        return Some(Err(e));
                    }
                }
            }
            for expr in &ident.bracket_indices {
                if let Err(e) = resolver.discover_expr_function_calls(expr, ctx) {
                    return Some(Err(e));
                }
            }
            None
        }
    }
    fn build_expr_recipe(
        &self,
        _resolver: &TypeResolver,
        ctx: &ScopeContext,
        deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        let ident = &self.identifier;
        let num_groups = ident.paren_groups.len();
        let is_var = ctx.variables.contains_key(&ident.name);
        let is_func = ctx.functions.contains_key(&ident.name);

        // Apply the same disambiguation logic as discover_expr_function_calls():
        // - 0 paren groups → variable
        // - 1 group, multiple args → function call only if arity matches; else variable
        // - 1 group, 1 arg → prefer variable if it exists, else function
        // - ≥2 groups → variable (multi-dim indexing)
        let is_function_call = match num_groups {
            0 => false,
            1 => {
                let num_args = ident.paren_groups[0].len();
                if num_args > 1 {
                    let func_accepts = is_func
                        && ctx
                            .functions
                            .get(&ident.name)
                            .map(|(_, args)| args.len() == num_args)
                            .unwrap_or(false);
                    if func_accepts {
                        true
                    } else if is_var {
                        false
                    } else {
                        is_func
                    }
                } else {
                    // Single arg: variable wins if it exists, else function
                    !is_var && is_func
                }
            }
            _ => false,
        };

        if is_function_call {
            if let Some((ret_slot, _)) = ctx.functions.get(&ident.name) {
                deps.insert(ret_slot.clone());
                ExprRecipe::Variable(ret_slot.clone())
            } else {
                let hint = find_case_similar(&ident.name, ctx.functions.keys());
                ExprRecipe::Unknown(Some(format!(
                    "undeclared function '{}'{}",
                    ident.name, hint
                )))
            }
        } else if let Some(slot) = ctx.variables.get(&ident.name) {
            deps.insert(slot.clone());
            let num_indices = ident.num_index_dimensions();
            if num_indices > 0 {
                ExprRecipe::IndexedVariable(slot.clone(), num_indices)
            } else {
                ExprRecipe::Variable(slot.clone())
            }
        } else {
            let hint = find_case_similar(&ident.name, ctx.variables.keys());
            ExprRecipe::Unknown(Some(format!(
                "undeclared variable '{}'{}",
                ident.name, hint
            )))
        }
    }
    fn as_bare_variable_name(&self) -> Option<&str> {
        if self.identifier.paren_groups.is_empty() && self.identifier.bracket_indices.is_empty() {
            Some(&self.identifier.name)
        } else {
            None
        }
    }
}
impl Transpile for VarExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        match self.classify(context)? {
            VarExprKind::FunctionCall => {
                // Delegate to function call helper — args are the single paren group
                function_call_transpile_helper(
                    &self.identifier.name,
                    &self.identifier.paren_groups[0],
                    context,
                )
            }
            VarExprKind::Variable => {
                let ident_output = self.identifier.transpile(context).map_err(|e| {
                    e.into_iter()
                        .map(|err| {
                            err.context(format!(
                                "...while transpiling variable identifier '{}'",
                                self.identifier.name
                            ))
                        })
                        .collect::<Vec<Error>>()
                })?;

                let var_data =
                    context
                        .variables
                        .get(&self.identifier.name)
                        .ok_or(vec![anyhow::anyhow!(
                            "Variable '{}' is not defined in this scope!{}",
                            self.identifier.name,
                            context.variable_hint(&self.identifier.name)
                        )])?;
                let var_type = var_data.data.r#type;

                // For indexed access, rosy_get() already returns &T — no
                // extra reference sigil needed regardless of scope or Copy-ness.
                let has_indices = self.identifier.num_index_dimensions() > 0;
                let (reference, value_kind) = if has_indices {
                    ("", ValueKind::Ref)
                } else {
                    match var_data.scope {
                        VariableScope::Local => {
                            if var_type.is_copy() {
                                ("", ValueKind::Owned) // Copy local: just `X`, value is copied
                            } else {
                                ("&", ValueKind::Ref) // non-Copy local: `&X`, reference
                            }
                        }
                        VariableScope::Arg => ("", ValueKind::Ref), // already a reference
                        VariableScope::Higher => ("", ValueKind::Ref), // already a reference
                    }
                };
                Ok(TranspilationOutput {
                    serialization: format!("{}{}", reference, ident_output.serialization),
                    requested_variables: ident_output.requested_variables,
                    value_kind,
                })
            }
        }
    }
}

pub fn function_call_transpile_helper(
    name: &String,
    args: &[Expr],
    context: &mut TranspilationInputContext,
) -> Result<TranspilationOutput, Vec<Error>> {
    // Start by checking that the function exists
    let func_context = match context.functions.get(name) {
        Some(ctx) => ctx,
        None => {
            let hint = TranspilationInputContext::case_hint(name, context.functions.keys());
            return Err(vec![anyhow!(
                "Function '{}' is not defined in this scope!{}",
                name,
                hint
            )]);
        }
    }
    .clone();

    // Check that the number of arguments is correct
    if func_context.args.len() != args.len() {
        return Err(vec![anyhow!(
            "Function '{}' expects {} arguments, but {} were provided!",
            name,
            func_context.args.len(),
            args.len()
        )]);
    }
    let mut errors = Vec::new();
    let mut requested_variables = BTreeSet::new();
    let mut serialized_args = Vec::new();
    // Serialize the requested variables from the function context
    for var in &func_context.requested_variables {
        let var_data = context.variables.get(var).ok_or(vec![anyhow!(
            "Could not find variable '{}' requested by function '{}'",
            var,
            name
        )])?;

        let serialized_arg = match var_data.scope {
            VariableScope::Higher => var.to_string(),
            VariableScope::Arg => var.to_string(),
            VariableScope::Local => format!("&mut {}", var),
        };
        serialized_args.push(serialized_arg);
    }

    // Two related call-site borrow hazards both lower to the same fix —
    // pre-evaluating offending args into fresh local temps ahead of the
    // call — so they share one prelude_decls accumulator:
    //
    //   (a) Bare-variable duplicates. `F(I, I)` would emit two `&mut <I>`
    //       references to the same memory; rustc rejects with E0499
    //       ("cannot borrow as mutable more than once").
    //
    //   (b) Mixed mut/shared borrows in one call. `F(A, LENGTH(A))` would
    //       emit `__fn_F(&mut *A, &mut RosyLENGTH::rosy_length(&*A))`,
    //       where `&*A` (inside the third arg's expression) overlaps the
    //       `&mut *A` from the first arg; rustc rejects with E0502.
    //
    // The remedy in both cases is to materialize the offending value into a
    // local first: the value's borrows on inputs are released when the let
    // binding completes, so the subsequent call sees only the temp's
    // `&mut <temp>`. Bare-var first-occurrences keep their original `&mut`
    // (so a function that genuinely mutates the arg still propagates that
    // mutation back to the caller's binding).
    let mut first_occurrence: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut prelude_decls: Vec<String> = Vec::new();
    let mut prelude_overrides: std::collections::HashMap<usize, String> =
        std::collections::HashMap::new();
    // Writebacks for dup-arg clones — see procedure_call/mod.rs for the
    // matching rationale. Without this, a function called with aliased
    // mutable args (`F(A, A)`) silently drops the mutation that would have
    // landed in the second occurrence.
    let mut writeback_decls: Vec<String> = Vec::new();

    // Pass 1 — record (a) bare-variable duplicates.
    for (i, arg_expr) in args.iter().enumerate() {
        if let Some(arg_name) = arg_expr.as_bare_variable_name() {
            if first_occurrence.contains_key(arg_name) {
                if let Some(var_data) = context.variables.get(arg_name) {
                    let temp_name = format!("__rosy_dup_arg_{}", i);
                    let (value_expr, writeback) = match var_data.scope {
                        VariableScope::Higher | VariableScope::Arg => (
                            format!("(*{}).clone()", arg_name),
                            format!("*{} = {};", arg_name, temp_name),
                        ),
                        VariableScope::Local => (
                            format!("{}.clone()", arg_name),
                            format!("{} = {};", arg_name, temp_name),
                        ),
                    };
                    prelude_decls.push(format!("let mut {} = {};", temp_name, value_expr));
                    prelude_overrides.insert(i, format!("&mut {}", temp_name));
                    writeback_decls.push(writeback);
                }
            } else {
                first_occurrence.insert(arg_name.to_string(), i);
            }
        }
    }

    // Add the manual arguments
    for (i, arg_expr) in args.iter().enumerate() {
        match arg_expr.transpile(context) {
            Ok(arg_output) => {
                // Check the type is correct
                let provided_type = arg_expr.type_of(context).map_err(|e| vec![e])?;
                let expected_type = func_context
                    .args
                    .get(i)
                    .ok_or(vec![anyhow!(
                        "Function '{}' expects {} arguments, but {} were provided!",
                        name,
                        func_context.args.len(),
                        args.len()
                    )])?
                    .r#type;
                if provided_type != expected_type {
                    errors.push(anyhow!(
                        "Function '{}' expects argument {} ('{}') to be of type '{}', but type '{}' was provided!",
                        name, i+1, func_context.args[i].name, expected_type, provided_type
                    ));
                } else if let Some(override_serialization) = prelude_overrides.remove(&i) {
                    // (a) bare-variable duplicate: use the pre-staged temp.
                    serialized_args.push(override_serialization);
                    requested_variables.extend(arg_output.requested_variables);
                } else if arg_expr.as_bare_variable_name().is_some() {
                    // Bare-variable first-occurrence: keep the natural
                    // `&mut <var>` so any in/out mutation flows back to
                    // the caller's binding.
                    serialized_args.push(arg_output.as_mut_ref());
                    requested_variables.extend(arg_output.requested_variables);
                } else {
                    // (b) Non-bare arg expression: pre-evaluate into a
                    // temp so any borrows the expression takes are released
                    // before the call begins. This is also where literals
                    // / operator results / function-call returns / indexed
                    // access expressions land — all benign individually, but
                    // some carry borrows of variables that other args also
                    // reference.
                    let temp_name = format!("__rosy_arg_tmp_{}", i);
                    let value_serial = arg_output.as_owned(&expected_type);
                    prelude_decls.push(format!("let mut {} = {};", temp_name, value_serial));
                    serialized_args.push(format!("&mut {}", temp_name));
                    requested_variables.extend(arg_output.requested_variables);
                }
            }
            Err(arg_errors) => {
                for e in arg_errors {
                    errors.push(e.context(format!(
                        "...while transpiling argument {} for function '{}'",
                        i + 1,
                        name
                    )));
                }
            }
        }
    }

    // Serialize the function call.
    // Uses the `__fn_` prefix to match the generated Rust function name
    // (the prefix avoids shadowing by the implicit return variable).
    let rust_fn_name = format!("__fn_{}", name);
    let serialization = if prelude_decls.is_empty() && writeback_decls.is_empty() {
        format!(
            "({}({})? as {})",
            rust_fn_name,
            serialized_args.join(", "),
            func_context.return_type.as_rust_type()
        )
    } else if writeback_decls.is_empty() {
        // Wrap in a block so the temp locals don't leak into the surrounding scope.
        format!(
            "{{ {} ({}({})? as {}) }}",
            prelude_decls.join(" "),
            rust_fn_name,
            serialized_args.join(", "),
            func_context.return_type.as_rust_type()
        )
    } else {
        // Capture the return value, run writebacks, then yield the value.
        // The trailing expression (no semicolon) makes the block evaluate to
        // __rosy_fn_ret, preserving the function-call-as-expression semantics.
        format!(
            "{{ {} let __rosy_fn_ret = ({}({})? as {}); {} __rosy_fn_ret }}",
            prelude_decls.join(" "),
            rust_fn_name,
            serialized_args.join(", "),
            func_context.return_type.as_rust_type(),
            writeback_decls.join(" "),
        )
    };
    if errors.is_empty() {
        // Transitive global capture: every variable the called function
        // captures must also be visible to the caller, otherwise the
        // generated Rust call `__fn_F(PI, ...)` references a `PI` that
        // isn't in the caller's parameter list. The body transpiler
        // collects this requested_variables set per statement and rolls it
        // up into the enclosing function/procedure's own signature, so
        // captures flow along the call graph by simple union at each link.
        requested_variables.extend(func_context.requested_variables.iter().cloned());
        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            value_kind: ValueKind::Owned,
        })
    } else {
        Err(errors)
    }
}
