//! # Assignment Statement
//!
//! Assigns the result of an expression to a variable (optionally indexed).
//!
//! ## Syntax
//!
//! ```text
//! name := expr;
//! name(i) := expr;          { indexed assignment }
//! name(i)(j) := expr;       { multi-dim indexed assignment }
//! ```
//!
//! ## Type Checking
//!
//! The right-hand side expression must be type-compatible with the target
//! variable. Indexed assignments check that the resulting element type matches.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/assign.rosy"))]
//! ```

use std::collections::{BTreeSet, HashSet};

use super::super::super::{TranspilationInputContext, TranspilationOutput, Transpile};
use crate::errors::RosyError;
use crate::{
    ast::*,
    program::{
        expressions::{Expr, core::variable_identifier::VariableIdentifier},
        statements::SourceLocation,
    },
    resolve::*,
    transpile::*,
};
use anyhow::{Context, Error, Result, anyhow, ensure};
use rosy_lib::{RosyBaseType, RosyType};

/// `COORD(1) := COORD(1) & DA(...)` nests one extra DA/CD dimension on the
/// container (like `A(1) := A(1) & 3` on `(VE n)`). Same base, rhs is one
/// dim thicker than the peeled/old type.
fn da_concat_nest_promote(elem_or_old: &RosyType, rhs_or_new: &RosyType) -> bool {
    matches!(
        elem_or_old.base_type,
        RosyBaseType::DA | RosyBaseType::CD
    ) && elem_or_old.base_type == rhs_or_new.base_type
        && rhs_or_new.dimensions == elem_or_old.dimensions + 1
}

fn bump_da_array_nesting(resolver: &mut TypeResolver, var_slot: &TypeSlot, new_type: RosyType) {
    if let Some(node) = resolver.nodes.get_mut(var_slot) {
        node.resolved = Some(new_type);
        match &mut node.rule {
            ResolutionRule::Explicit(t) => {
                t.dimensions = new_type.dimensions;
            }
            ResolutionRule::InferredFrom { recipe, reason } => {
                *recipe = ExprRecipe::Literal(new_type);
                reason.clear();
                reason.push_str("da/cd indexed concat nest");
            }
            _ => {}
        }
    }
}

/// AST node for the assignment statement `name := expr;`.
#[derive(Debug)]
pub struct AssignStatement {
    pub identifier: VariableIdentifier,
    /// `None` when RHS is `.` (clear/reset), `Some` for normal expressions.
    pub value: Option<Expr>,
}

impl FromRule for AssignStatement {
    fn from_rule(pair: pest::iterators::Pair<crate::ast::Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == crate::ast::Rule::assignment,
            "Expected `assignment` rule when building assignment statement, found: {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();

        let lhs = inner
            .next()
            .context("Missing first token `variable_name`!")?;
        let identifier = VariableIdentifier::from_rule(lhs)
            .context("...while building variable identifier for assignment statement")?
            .ok_or_else(|| {
                anyhow::anyhow!("Expected variable identifier for assignment statement")
            })?;

        let rhs_pair = inner
            .next()
            .context("Missing second token in assignment!")?;
        let value = if rhs_pair.as_rule() == crate::ast::Rule::empty_literal {
            None
        } else {
            Some(
                Expr::from_rule(rhs_pair)
                    .context("Failed to build expression for assignment statement!")?
                    .ok_or_else(|| {
                        anyhow::anyhow!("Expected expression for assignment statement")
                    })?,
            )
        };

        Ok(Some(AssignStatement { identifier, value }))
    }
}
impl TranspileableStatement for AssignStatement {
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
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        source_location: SourceLocation,
    ) -> Option<Result<()>> {
        // Clear assignments (`:= .`) don't affect type resolution
        let value = match &self.value {
            Some(v) => v,
            None => return Some(Ok(())),
        };

        // Discover function call sites within the RHS expression
        if let Err(e) = resolver.discover_expr_function_calls(value, ctx) {
            return Some(Err(e.context(
                "...while discovering function call dependencies in assignment statement",
            )));
        }

        let var_name = &self.identifier.name;
        let var_slot = match ctx.variables.get(var_name) {
            Some(s) => s.clone(),
            None => return Some(Ok(())), // unknown variable, skip
        };

        // Build a recipe for the RHS expression and collect its dependencies
        let mut deps = HashSet::new();
        let recipe = resolver.build_expr_recipe(value, ctx, &mut deps);

        if let Some(node) = resolver.nodes.get(&var_slot) {
            if let Some(&resolved) = node.resolved.as_ref() {
                // Already has an explicit type — check that the new
                // assignment is compatible (if evaluable now).
                // Account for indexing on the LHS: X[I, J] := expr
                // means we're assigning to a sub-element, so reduce
                // the declared dimensions by the number of indices.
                let mut explicit_type = resolved;
                let num_indices = self.identifier.num_index_dimensions();
                if num_indices > 0
                    && explicit_type.base_type == RosyBaseType::VE
                    && explicit_type.dimensions == 0
                {
                    explicit_type = RosyType::RE();
                } else {
                    explicit_type.dimensions = explicit_type.dimensions.saturating_sub(num_indices);
                }
                if explicit_type.is_any() {
                    return Some(Ok(()));
                }
                if let Ok(new_type) = resolver.evaluate_recipe(&recipe)
                    && new_type != explicit_type
                    && !new_type.is_any()
                {
                    if da_concat_nest_promote(&explicit_type, &new_type) {
                        let mut nested = resolved;
                        nested.dimensions += 1;
                        bump_da_array_nesting(resolver, &var_slot, nested);
                        return Some(Ok(()));
                    }
                    // VARIABLE (DA) X; X := 1 is fine. declared RE stays RE.
                    if re_da_assignment_type(explicit_type, new_type) == Some(explicit_type) {
                        return Some(Ok(()));
                    }
                    if re_ve_assignment_type(explicit_type, new_type) == Some(explicit_type) {
                        return Some(Ok(()));
                    }
                    if crate::syntax_config::is_cosy_syntax() {
                        if let Some(node) = resolver.nodes.get_mut(&var_slot) {
                            node.rule = ResolutionRule::InferredFrom {
                                recipe: ExprRecipe::Literal(RosyType::ANY()),
                                reason: "reused as multiple types".to_string(),
                            };
                            node.resolved = Some(RosyType::ANY());
                            node.depends_on.clear();
                        }
                        return Some(Ok(()));
                    }
                    let scope_str = if ctx.scope_path.is_empty() {
                        "global scope".to_string()
                    } else {
                        format!("'{}'", ctx.scope_path.join(" > "))
                    };
                    let decl_hint = node
                        .declared_at
                        .as_ref()
                        .map(|loc| format!("\n│  📍 Declared at: {}", loc))
                        .unwrap_or_default();
                    let assign_hint = format!("\n│  📍 Assigned at: {}", source_location);
                    // Mode-aware hint for common VE↔RE patterns
                    let ve_hint = {
                        let re = RosyType::RE();
                        let ve = RosyType::VE();
                        if explicit_type == ve && new_type == re {
                            if crate::syntax_config::is_cosy_syntax() {
                                format!(
                                    "\n│\n\
                                         │  📖 In COSY, RE values were implicitly upcast to VE.\n\
                                         │     In Rosy, use an explicit conversion instead:\n\
                                         │     • Wrap the value:            {} := VE(<expr>);\n\
                                         │     • Build via concatenation:   {} := 0 & 1 & 2;",
                                    var_name, var_name
                                )
                            } else {
                                format!(
                                    "\n│\n\
                                         │  📖 To assign a scalar to a VE variable, wrap it explicitly:\n\
                                         │     • Wrap the value:            {} := VE(<expr>);\n\
                                         │     • Build via concatenation:   {} := 0 & 1 & 2;",
                                    var_name, var_name
                                )
                            }
                        } else if (explicit_type == re && new_type == ve)
                            || (explicit_type == ve && new_type == re)
                        {
                            if crate::syntax_config::is_cosy_syntax() {
                                format!(
                                    "\n│\n\
                                         │  📖 Common COSY vector patterns:\n\
                                         │     • Build via concatenation:  {} := 0 & 1 & 2;\n\
                                         │     • Pre-sized array:          VARIABLE {} <mem> <dim>;",
                                    var_name, var_name
                                )
                            } else {
                                format!(
                                    "\n│\n\
                                         │  📖 Common vector patterns:\n\
                                         │     • Build via concatenation:  {} := 0 & 1 & 2;\n\
                                         │     • Declare as array:         VARIABLE (RE <dim>) {};",
                                    var_name, var_name
                                )
                            }
                        } else {
                            String::new()
                        }
                    };
                    let msg = format!(
                        "\n╭─ Type Conflict ──────────────────────────────────────────\n\
                                │\n\
                                │  Variable '{}' (in {}) is declared as {} but is\n\
                                │  assigned a value of type {}.{}{}\n\
                                │\n\
                                │  💡 Either:\n\
                                │     • Change the explicit type to match the assignment, or\n\
                                │     • Split into separate variables: {}_{:?}  and  {}_{:?}\n\
                                │{}\n\
                                ╰──────────────────────────────────────────────────────────",
                        var_name,
                        scope_str,
                        explicit_type,
                        new_type,
                        decl_hint,
                        assign_hint,
                        var_name,
                        explicit_type.base_type,
                        var_name,
                        new_type.base_type,
                        ve_hint,
                    );
                    return Some(Err(RosyError::at(source_location.clone(), msg).into()));
                }
                return Some(Ok(())); // already has explicit type, no inference needed
            }
        } else {
            return Some(Ok(()));
        }

        // Check for conflicting re-assignment: if a previous assignment
        // already established an inference recipe, verify the new one
        // produces the same type (when both are evaluable).
        let has_existing_rule = matches!(
            resolver.nodes.get(&var_slot).map(|n| &n.rule),
            Some(ResolutionRule::InferredFrom { .. })
        );

        if has_existing_rule {
            let old_recipe = if let Some(node) = resolver.nodes.get(&var_slot) {
                if let ResolutionRule::InferredFrom { recipe: ref r, .. } = node.rule {
                    Some(r.clone())
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(ref old_recipe) = old_recipe {
                // Try to evaluate both recipes. If the new recipe is
                // self-referential (e.g. Y:=Y&I), evaluate_recipe will
                // fail because Y isn't resolved yet. In that case,
                // temporarily resolve Y to the old type so we can
                // evaluate what the new assignment produces.
                //
                // Account for LHS indexing: MAP1(2) := expr means the
                // recipe should be wrapped with WithDimensions, just like
                // the first assignment does at the bottom of this function.
                let num_indices = self.identifier.num_index_dimensions();
                let dimensioned_recipe = if num_indices > 0 {
                    ExprRecipe::WithDimensions(Box::new(recipe.clone()), num_indices)
                } else {
                    recipe.clone()
                };
                let mut old_type_result = resolver.evaluate_recipe(old_recipe);
                let mut new_type_result = resolver.evaluate_recipe(&dimensioned_recipe);

                // If either recipe can't be evaluated yet (e.g. X:=10.5*J
                // where J has a known recipe but isn't resolved, or
                // Z:=Z+X*Y where X/Y are inferred but not yet resolved),
                // temporarily resolve inferred dependencies so the conflict
                // checker can see RE↔VE coercion vs a same-type mutation.
                // Non-leaf deps (DFX := F % 1, then F := DFX % (-1)) are
                // included: assume this slot has its old type, then walk
                // recipes.
                let mut temp_leaf_slots: Vec<TypeSlot> = Vec::new();
                if old_type_result.is_err() || new_type_result.is_err() {
                    let all_deps: HashSet<TypeSlot> = {
                        let mut d = resolver
                            .nodes
                            .get(&var_slot)
                            .map(|n| n.depends_on.clone())
                            .unwrap_or_default();
                        d.extend(deps.iter().cloned());
                        d
                    };

                    let mut visiting = HashSet::new();
                    visiting.insert(var_slot.clone());
                    for dep_slot in &all_deps {
                        if *dep_slot == var_slot {
                            continue;
                        }
                        temp_resolve_inferred(
                            resolver,
                            dep_slot,
                            &mut temp_leaf_slots,
                            &mut visiting,
                        );
                    }

                    if !temp_leaf_slots.is_empty() {
                        old_type_result = resolver.evaluate_recipe(old_recipe);
                        new_type_result = resolver.evaluate_recipe(&dimensioned_recipe);
                    }

                    // Self-referential later assignment: pin this slot to
                    // the first assignment's type, then re-type the new recipe
                    // (Y:=Y&I, DFX:=DFX%1).
                    if new_type_result.is_err() {
                        if old_type_result.is_err() {
                            old_type_result = resolver.evaluate_recipe(old_recipe);
                        }
                        if let Ok(old_type) = old_type_result {
                            if resolver
                                .nodes
                                .get(&var_slot)
                                .and_then(|n| n.resolved)
                                .is_none()
                            {
                                if let Some(node) = resolver.nodes.get_mut(&var_slot) {
                                    node.resolved = Some(old_type);
                                }
                                temp_leaf_slots.push(var_slot.clone());
                            }
                            visiting.remove(&var_slot);
                            for dep_slot in &all_deps {
                                if *dep_slot == var_slot {
                                    continue;
                                }
                                temp_resolve_inferred(
                                    resolver,
                                    dep_slot,
                                    &mut temp_leaf_slots,
                                    &mut visiting,
                                );
                            }
                            new_type_result =
                                resolver.evaluate_recipe(&dimensioned_recipe);
                            old_type_result = Ok(old_type);
                        }
                    }
                }

                // Undo temporary resolutions
                for slot in &temp_leaf_slots {
                    if let Some(node) = resolver.nodes.get_mut(slot) {
                        node.resolved = None;
                    }
                }

                if let (Ok(old_type), Ok(new_type)) =
                    (old_type_result.as_ref(), new_type_result.as_ref())
                {
                    let old_type = *old_type;
                    let new_type = *new_type;
                    if old_type != new_type {
                    if da_concat_nest_promote(&old_type, &new_type) {
                        bump_da_array_nesting(resolver, &var_slot, new_type);
                        return Some(Ok(()));
                    }
                    if let Some(promoted) = re_da_assignment_type(old_type, new_type) {
                        if promoted != old_type
                            && let Some(node) = resolver.nodes.get_mut(&var_slot)
                        {
                            node.rule = ResolutionRule::InferredFrom {
                                recipe: ExprRecipe::Literal(promoted),
                                reason: "RE promoted to DA".to_string(),
                            };
                            node.depends_on.clear();
                        }
                        return Some(Ok(()));
                    }
                    if let Some(promoted) = re_ve_assignment_type(old_type, new_type) {
                        if promoted != old_type
                            && let Some(node) = resolver.nodes.get_mut(&var_slot)
                        {
                            node.rule = ResolutionRule::InferredFrom {
                                recipe: ExprRecipe::Literal(promoted),
                                reason: "RE promoted to VE".to_string(),
                            };
                            node.resolved = Some(promoted);
                            node.depends_on.clear();
                        }
                        return Some(Ok(()));
                    }
                    // Cosy cells are untyped. Rosy ANY only for 0-d base conflicts.
                    let any_ok = crate::syntax_config::is_cosy_syntax()
                        || (old_type.dimensions == 0 && new_type.dimensions == 0);
                    if any_ok
                        && (crate::syntax_config::is_cosy_syntax()
                            || old_type.is_any()
                            || new_type.is_any()
                            || old_type.base_type != new_type.base_type)
                    {
                        if let Some(node) = resolver.nodes.get_mut(&var_slot) {
                            node.rule = ResolutionRule::InferredFrom {
                                recipe: ExprRecipe::Literal(RosyType::ANY()),
                                reason: "reused as multiple types".to_string(),
                            };
                            node.resolved = Some(RosyType::ANY());
                            node.depends_on.clear();
                        }
                        return Some(Ok(()));
                    }
                    let scope_str = if ctx.scope_path.is_empty() {
                        "global scope".to_string()
                    } else {
                        format!("'{}'", ctx.scope_path.join(" > "))
                    };
                    let first_assign_hint = resolver
                        .nodes
                        .get(&var_slot)
                        .and_then(|n| n.assigned_at.as_ref())
                        .map(|loc| format!("\n│  📍 First assigned at:  {}", loc))
                        .unwrap_or_default();
                    let second_assign_hint =
                        format!("\n│  📍 Then assigned at:   {}", source_location);
                    // Migration hint for RE→VE pattern
                    let ve_hint = {
                        let re = RosyType::RE();
                        let ve = RosyType::VE();
                        if old_type == re && new_type == ve {
                            if crate::syntax_config::is_cosy_syntax() {
                                format!(
                                    "\n│\n\
                                         │  📖 In COSY, RE values were implicitly upcast to VE.\n\
                                         │     In Rosy, make the first assignment a VE explicitly:\n\
                                         │     • Wrap the value:            {} := VE(<expr>);\n\
                                         │     • Build via concatenation:   {} := 0 & 1 & 2;",
                                    var_name, var_name
                                )
                            } else {
                                format!(
                                    "\n│\n\
                                         │  📖 To make '{}' a VE, ensure the first assignment is a VE:\n\
                                         │     • Wrap the value:            {} := VE(<expr>);\n\
                                         │     • Build via concatenation:   {} := 0 & 1 & 2;",
                                    var_name, var_name, var_name
                                )
                            }
                        } else {
                            String::new()
                        }
                    };
                    let msg = format!(
                        "\n╭─ Type Conflict ──────────────────────────────────────────\n\
                                │\n\
                                │  Variable '{}' (in {}) is assigned conflicting types:\n\
                                │     • First inferred as:  {}\n\
                                │     • Then assigned as:   {}{}{}\n\
                                │\n\
                                │  Type elision requires each variable to have exactly one type.\n\
                                │\n\
                                │  💡 Either:\n\
                                │     • Add an explicit type:  VARIABLE ({:?}) {} ;\n\
                                │     • Split into separate variables: {}_{:?}  and  {}_{:?}\n\
                                │{}\n\
                                ╰──────────────────────────────────────────────────────────",
                        var_name,
                        scope_str,
                        old_type,
                        new_type,
                        first_assign_hint,
                        second_assign_hint,
                        old_type.base_type,
                        var_name,
                        var_name,
                        old_type.base_type,
                        var_name,
                        new_type.base_type,
                        ve_hint,
                    );
                    return Some(Err(RosyError::at(source_location.clone(), msg).into()));
                    }
                } else if crate::syntax_config::is_cosy_syntax()
                    && old_type_result.is_err()
                {
                    // First assignment still has unresolved deps (e.g. `X := 10^(-2)*J`
                    // before J is typed). Keep that recipe; do not lock the cell to ANY
                    // or `X := X & …` can never RE→VE promote.
                }
            }

            // Keep the first (non-self-referential) assignment's recipe.
            // Subsequent assignments are mutations — the variable's type
            // is established by its first assignment. This prevents false
            // cycles when a variable is re-assigned from a value that
            // transitively depends on itself (e.g. X1 := f(X3) after
            // X3 := g(X1)).
            return Some(Ok(()));
        }

        if let Some(node) = resolver.nodes.get_mut(&var_slot) {
            // Remove self-reference from deps if present — the variable's
            // type is being established by this very assignment
            deps.remove(&var_slot);
            // Account for indexing on the LHS: X[I, J] := expr means
            // the variable is a 2D array of whatever the RHS type is.
            let num_indices = self.identifier.num_index_dimensions();
            let recipe = if num_indices > 0 {
                ExprRecipe::WithDimensions(Box::new(recipe), num_indices)
            } else {
                recipe
            };
            node.rule = ResolutionRule::InferredFrom {
                recipe,
                reason: "inferred from assignment".to_string(),
            };
            node.depends_on = deps;
            node.assigned_at = Some(source_location);
        }

        Some(Ok(()))
    }
    fn hydrate_resolved_types(
        &mut self,
        _resolver: &TypeResolver,
        _current_scope: &[String],
    ) -> Option<Result<()>> {
        None
    }
}
impl Transpile for AssignStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Get the variable type
        let variable_type = self.identifier.type_of(context).map_err(|e| {
            vec![e.context("...while determining type of variable identifier for assignment")]
        })?;

        // Handle clear assignment (`:= .`)
        if self.value.is_none() {
            // Only allowed on VE or array types (dimensions > 0)
            let is_ve =
                variable_type.base_type == RosyBaseType::VE && variable_type.dimensions == 0;
            let is_array = variable_type.dimensions > 0;
            if !is_ve && !is_array {
                return Err(vec![anyhow!(
                    "Cannot use '.' (clear) on variable '{}' of type '{}' — only VE or array types can be cleared.",
                    self.identifier.name,
                    variable_type
                )]);
            }

            let mut requested_variables = BTreeSet::new();
            let ident_output = self.identifier.transpile(context).map_err(|e| {
                e.into_iter()
                    .map(|err| {
                        err.context(format!(
                            "...while transpiling identifier for clear assignment to '{}'",
                            self.identifier.name
                        ))
                    })
                    .collect::<Vec<Error>>()
            })?;
            requested_variables.extend(ident_output.requested_variables.iter().cloned());

            // Generate the empty value for the type
            let empty_value = if is_ve {
                "vec![]".to_string()
            } else {
                // For arrays, create nested empty vecs
                let mut result = "vec![]".to_string();
                for _ in 1..variable_type.dimensions {
                    result = format!("vec![{}]", result);
                }
                result
            };

            let dereference = match context
                .variables
                .get(&self.identifier.name)
                .ok_or(vec![anyhow::anyhow!(
                    "Variable '{}' is not defined in this scope!{}",
                    self.identifier.name,
                    context.variable_hint(&self.identifier.name)
                )])?
                .scope
            {
                VariableScope::Local => "",
                VariableScope::Arg => "*",
                VariableScope::Higher => {
                    requested_variables.insert(self.identifier.name.clone());
                    "*"
                }
            };
            let serialization = format!(
                "{}{} = {};",
                dereference, ident_output.serialization, empty_value
            );
            return Ok(TranspilationOutput {
                serialization,
                requested_variables,
                ..Default::default()
            });
        }

        let value = self.value.as_ref().unwrap();
        let value_type = value.type_of(context).map_err(|e| {
            vec![e.context("...while determining type of value expression for assignment")]
        })?;
        let da_singleton_wrap = matches!(
            variable_type.base_type,
            RosyBaseType::DA | RosyBaseType::CD
        ) && variable_type.base_type == value_type.base_type
            && variable_type.dimensions == value_type.dimensions + 1;
        if variable_type != value_type
            && !variable_type.is_any()
            && !value_type.is_any()
            && !da_singleton_wrap
            && re_da_assignment_type(variable_type, value_type) != Some(variable_type)
            && re_ve_assignment_type(variable_type, value_type) != Some(variable_type)
        {
            return Err(vec![anyhow!(
                "Cannot assign value of type '{}' to variable '{}' of type '{}'!",
                value_type,
                self.identifier.name,
                variable_type
            )]);
        }

        // `X := X & expr` / `X(I) := X(I) & expr` → push/extend on the cell
        if let Ok((dest, idx_serials, mut dest_vars)) =
            assignment_append_dest(&self.identifier, context)
            && let Some(result) = value.inner.try_inplace_append(
                &self.identifier.name,
                &idx_serials,
                &dest,
                context,
            )
        {
            return match result {
                Ok(mut out) => {
                    out.requested_variables.append(&mut dest_vars);
                    Ok(out)
                }
                Err(e) => Err(e),
            };
        }

        let mut requested_variables = BTreeSet::new();
        let mut errors = Vec::new();

        // Serialize the identifier
        let ident_output = match self.identifier.transpile(context) {
            Ok(output) => output,
            Err(vec_err) => {
                for err in vec_err {
                    errors.push(err.context(format!(
                        "...while transpiling identifier expression for assigment to '{}'",
                        self.identifier.name
                    )));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(ident_output.requested_variables.iter().cloned());
        let serialized_identifier = ident_output.serialization;

        // Serialize the value
        let value_output = match value.transpile(context) {
            Ok(output) => output,
            Err(value_errors) => {
                for err in value_errors {
                    errors.push(err.context(format!(
                        "...while transpiling value expression for assignment to '{}'",
                        self.identifier.name
                    )));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(value_output.requested_variables.iter().cloned());

        let num_indices = self.identifier.num_index_dimensions();
        let container_ty = context
            .variables
            .get(&self.identifier.name)
            .map(|v| v.data.r#type);
        let container_rosyvalue = container_ty
            .as_ref()
            .map(|t| t.is_any() || t.as_rust_type().contains("RosyValue"))
            .unwrap_or(false);
        let serialized_value = if (variable_type.is_any()
            || (num_indices > 0 && container_rosyvalue))
            && !value_type.is_any()
        {
            format!("RosyValue::from({})", value_output.as_owned(&value_type))
        } else if (variable_type.is_any() || (num_indices > 0 && container_rosyvalue))
            && value_type.is_any()
        {
            let owned = value_output.as_owned(&value_type);
            if owned.contains("RosyValue") {
                owned
            } else {
                format!("RosyValue::from({owned})")
            }
        } else if !variable_type.is_any() && value_type.is_any() {
            format!(
                "({}).expect_{}()?",
                value_output.as_owned(&value_type),
                variable_type.base_type.to_string().to_lowercase()
            )
        } else if variable_type == RosyType::VE() && value_type == RosyType::RE() {
            format!("vec![{}]", value_output.as_owned(&value_type))
        } else if variable_type == RosyType::DA() && value_type == RosyType::RE() {
            format!("DA::from_coeff({})", value_output.as_owned(&value_type))
        } else if variable_type == RosyType::RE() && value_type != RosyType::RE() {
            format!("rosy_as_f64(&({}))", value_output.as_owned(&value_type))
        } else if variable_type == RosyType::ST() && value_type != RosyType::ST() {
            format!("RosyST::rosy_to_string(&{})", value_output.as_ref())
        } else if matches!(
            variable_type.base_type,
            RosyBaseType::DA | RosyBaseType::CD
        ) && variable_type.base_type == value_type.base_type
            && variable_type.dimensions == value_type.dimensions + 1
        {
            format!("vec![{}]", value_output.as_owned(&value_type))
        } else {
            value_output.as_owned(&variable_type)
        };

        // Serialize the entire assignment
        let var_scope = context
            .variables
            .get(&self.identifier.name)
            .ok_or(vec![anyhow::anyhow!(
                "Variable '{}' is not defined in this scope!{}",
                self.identifier.name,
                context.variable_hint(&self.identifier.name)
            )])?
            .scope
            .clone();
        let dereference = match var_scope {
            VariableScope::Local => "",
            VariableScope::Arg => "*",
            VariableScope::Higher => {
                requested_variables.insert(self.identifier.name.clone());
                "*"
            }
        };

        // Self-referential RHS handling: when the LHS needs dereferencing
        // (Arg or Higher scope, where the binding is `&mut T`), the natural
        //     *A = (... &*A ...);
        // simultaneously borrows A immutable (for the RHS reads) and mutable
        // (for the assignment), and rustc rejects with E0502 when the RHS
        // does in fact read A. Detecting "RHS reads LHS" exactly would mean
        // walking the AST or scanning the serialization for the identifier
        // name; either is brittle. Instead, ALWAYS route through a temp
        // when the LHS needs deref:
        //     { let __rosy_self_ref_tmp = (... &*A ...); *A = __rosy_self_ref_tmp; }
        // This is a no-op when the RHS doesn't read A — the extra binding
        // is optimized out — and a clean fix when it does. For a plain
        // Local `T` LHS no aliasing exists, so we skip the wrap.
        let needs_self_ref_temp = !dereference.is_empty();

        let serialization = if num_indices > 0 {
            // Indexed assignment: build a rosy_get_mut() chain.
            // The mutable borrow is passed into the function, avoiding
            // borrow-checker conflicts with container[expr_that_borrows_container].
            let flat = self.identifier.flat_indices();
            let mut idx_exprs = Vec::new();
            for index_expr in flat.iter() {
                match index_expr.transpile(context) {
                    Ok(output) => {
                        requested_variables.extend(output.requested_variables.iter().cloned());
                        idx_exprs.push(output.as_value());
                    }
                    Err(vec_err) => {
                        for err in vec_err {
                            errors.push(err.context(format!(
                                "...while transpiling index expression for assignment to '{}'",
                                self.identifier.name
                            )));
                        }
                    }
                }
            }
            // Build nested rosy_get_mut(container, idx, "name") calls.
            // Local scope: owned value, needs &mut to borrow mutably.
            // Arg/Higher scope: already &mut T, pass directly (auto-reborrows).
            let rust_name = context.rust_ident(&self.identifier.name);
            let mut_ref = match var_scope {
                VariableScope::Local => format!("&mut {rust_name}"),
                VariableScope::Arg | VariableScope::Higher => rust_name,
            };
            let mut result = mut_ref;
            for idx_expr in &idx_exprs {
                result = format!(
                    "rosy_get_mut({result}, {expr}, \"{name}\")",
                    result = result,
                    expr = idx_expr,
                    name = self.identifier.name,
                );
            }
            if needs_self_ref_temp {
                format!(
                    "{{ let __rosy_self_ref_tmp = {value}; *{lhs} = (__rosy_self_ref_tmp).into(); }}",
                    value = serialized_value,
                    lhs = result,
                )
            } else {
                format!("*{} = ({}).into();", result, serialized_value)
            }
        } else if needs_self_ref_temp {
            format!(
                "{{ let __rosy_self_ref_tmp = {value}; {deref}{lhs} = __rosy_self_ref_tmp; }}",
                value = serialized_value,
                deref = dereference,
                lhs = serialized_identifier,
            )
        } else {
            format!(
                "{}{} = {};",
                dereference, serialized_identifier, serialized_value
            )
        };
        if errors.is_empty() {
            Ok(TranspilationOutput {
                serialization,
                requested_variables,
                ..Default::default()
            })
        } else {
            Err(errors)
        }
    }
}

/// Seed a vector with a real (`X := 0` then `X := X & I`), including the
/// indexed form (`COORD(1) := 0` then `COORD(1) := COORD(1) & x`).
fn re_ve_assignment_type(old: RosyType, new: RosyType) -> Option<RosyType> {
    if old.dimensions != new.dimensions {
        return None;
    }
    match (old.base_type, new.base_type) {
        (RosyBaseType::RE, RosyBaseType::VE) => Some(new),
        (RosyBaseType::VE, RosyBaseType::RE) => Some(old),
        _ => None,
    }
}

/// Temporarily resolve an inferred slot from its recipe, walking deps first.
fn temp_resolve_inferred(
    resolver: &mut TypeResolver,
    slot: &TypeSlot,
    temp_slots: &mut Vec<TypeSlot>,
    visiting: &mut HashSet<TypeSlot>,
) {
    if visiting.contains(slot) {
        return;
    }
    if resolver
        .nodes
        .get(slot)
        .and_then(|n| n.resolved)
        .is_some()
    {
        return;
    }
    let is_unresolved = resolver
        .nodes
        .get(slot)
        .is_some_and(|n| matches!(n.rule, ResolutionRule::Unresolved));
    if is_unresolved && crate::syntax_config::is_cosy_syntax() {
        // Fox untyped cells (RERAN dests, unused-until-now names) default to
        // RE, same as topological_resolve. Needed so `X := 10^(-2)*J` then
        // `X := X & …` can see RE→VE instead of "not yet typed".
        if let Some(node) = resolver.nodes.get_mut(slot) {
            node.resolved = Some(RosyType::RE());
            temp_slots.push(slot.clone());
        }
        return;
    }
    let (recipe, deps) = match resolver.nodes.get(slot) {
        Some(node) => match &node.rule {
            ResolutionRule::InferredFrom { recipe, .. } => {
                (recipe.clone(), node.depends_on.clone())
            }
            _ => return,
        },
        None => return,
    };
    visiting.insert(slot.clone());
    for dep in &deps {
        temp_resolve_inferred(resolver, dep, temp_slots, visiting);
    }
    if let Ok(t) = resolver.evaluate_recipe(&recipe)
        && let Some(node) = resolver.nodes.get_mut(slot)
        && node.resolved.is_none()
    {
        node.resolved = Some(t);
        temp_slots.push(slot.clone());
    }
    visiting.remove(slot);
}

/// RE is a constant DA. same rank only. promotes the slot to DA when RE comes first.
fn re_da_assignment_type(old: RosyType, new: RosyType) -> Option<RosyType> {
    if old.dimensions != new.dimensions {
        return None;
    }
    match (old.base_type, new.base_type) {
        (RosyBaseType::RE, RosyBaseType::DA) => Some(new),
        (RosyBaseType::DA, RosyBaseType::RE) => Some(old),
        _ => None,
    }
}

fn assignment_append_dest(
    ident: &VariableIdentifier,
    context: &mut TranspilationInputContext,
) -> Result<(String, Vec<String>, BTreeSet<String>), Vec<Error>> {
    let mut requested_variables = BTreeSet::new();
    let mut idx_serials = Vec::new();
    for index_expr in ident.flat_indices() {
        let output = index_expr.transpile(context).map_err(|e| {
            e.into_iter()
                .map(|err| {
                    err.context(format!(
                        "...while transpiling index expression for assignment to '{}'",
                        ident.name
                    ))
                })
                .collect::<Vec<Error>>()
        })?;
        requested_variables.extend(output.requested_variables.iter().cloned());
        idx_serials.push(output.as_value());
    }
    let scope = context.variables.get(&ident.name).map(|v| v.scope.clone());
    if matches!(scope, Some(VariableScope::Higher)) {
        requested_variables.insert(ident.name.clone());
    }
    let rust_name = context.rust_ident(&ident.name);
    let dest = if idx_serials.is_empty() {
        match scope {
            Some(VariableScope::Arg | VariableScope::Higher) => format!("(*{rust_name})"),
            _ => format!("({rust_name})"),
        }
    } else {
        let mut cell = match scope {
            Some(VariableScope::Local) | None => format!("&mut {rust_name}"),
            Some(VariableScope::Arg | VariableScope::Higher) => rust_name,
        };
        for idx in &idx_serials {
            cell = format!(
                "rosy_get_mut({cell}, {idx}, \"{name}\")",
                name = ident.name
            );
        }
        cell
    };
    Ok((dest, idx_serials, requested_variables))
}

#[cfg(test)]
mod fox_reassignment_tests {
    use super::*;
    use crate::ast;
    use crate::program::{IncludeTracker, Program, syntax_config};
    use std::path::Path;

    fn resolve_fox(src: &str) -> TypeResolver {
        syntax_config::with_path(Some(Path::new("t.fox")), || {
            let program = ast::parse_source(src)
                .unwrap()
                .next()
                .expect("program");
            let mut ast = Program::from_rule_with_includes(
                program,
                Some(Path::new("t.fox")),
                &mut IncludeTracker::default(),
            )
            .unwrap()
            .expect("ast");
            TypeResolver::resolve(&mut ast).unwrap().0
        })
    }

    fn slot_type(resolver: &TypeResolver, name: &str) -> RosyType {
        resolver
            .nodes
            .iter()
            .find_map(|(slot, node)| match slot {
                TypeSlot::Variable(_, n) if n == name => node.resolved,
                _ => None,
            })
            .unwrap_or_else(|| panic!("no type for {name}"))
    }

    #[test]
    fn fox_self_ref_arith_using_other_re_vars_stays_re() {
        let resolver = resolve_fox(
            r#"
BEGIN ;
VARIABLE X 1 ;
VARIABLE Y 1 ;
VARIABLE Z 1 ;
X := 1.0 ;
Y := 2.0 ;
Z := 0.0 ;
LOOP I 1 3 ;
    Z := Z + X * Y - X / (Y + 1.0) + SQRT(X) ;
ENDLOOP ;
END ;
"#,
        );
        assert_eq!(slot_type(&resolver, "Z"), RosyType::RE());
        assert_eq!(slot_type(&resolver, "X"), RosyType::RE());
        assert_eq!(slot_type(&resolver, "Y"), RosyType::RE());
    }

    #[test]
    fn fox_re_then_concat_still_promotes_to_ve() {
        let resolver = resolve_fox(
            r#"
BEGIN ;
VARIABLE X 1 ;
X := 0 ;
LOOP I 1 3 ;
    X := X & I ;
ENDLOOP ;
END ;
"#,
        );
        assert_eq!(slot_type(&resolver, "X"), RosyType::VE());
    }

    #[test]
    fn fox_st_then_lo_still_becomes_any() {
        let resolver = resolve_fox(
            r#"
BEGIN ;
VARIABLE X 1 ;
X := 'hi' ;
X := LO(1) ;
END ;
"#,
        );
        assert_eq!(slot_type(&resolver, "X"), RosyType::ANY());
    }

    #[test]
    fn fox_da_then_derive_cycle_stays_da() {
        let resolver = resolve_fox(
            r#"
BEGIN ;
VARIABLE X 1 ;
VARIABLE F 300 ;
VARIABLE DFX 300 ;
VARIABLE NM 1 ;
DAINI 2 1 0 NM ;
X := DA(1) ;
F := X * X ;
F := F * F + X ;
DFX := F % 1 ;
DFX := DFX % 1 ;
F := DFX % (-1) ;
END ;
"#,
        );
        assert_eq!(slot_type(&resolver, "F"), RosyType::DA());
        assert_eq!(slot_type(&resolver, "DFX"), RosyType::DA());
        assert_eq!(slot_type(&resolver, "X"), RosyType::DA());
    }

    #[test]
    fn fox_re_expr_then_concat_promotes_to_ve() {
        let resolver = resolve_fox(
            r#"
BEGIN ;
VARIABLE J 1 ;
VARIABLE X 200 ;
RERAN J ;
X := 10^(-2)*J ;
LOOP I 2 5 ;
    RERAN J ;
    X := X & 10^(-2)*J ;
ENDLOOP ;
END ;
"#,
        );
        assert_eq!(slot_type(&resolver, "X"), RosyType::VE());
        assert_eq!(slot_type(&resolver, "J"), RosyType::RE());
    }

    #[test]
    fn fox_indexed_re_then_concat_promotes_ve_array() {
        let resolver = resolve_fox(
            r#"
BEGIN ;
PROCEDURE RUN ;
VARIABLE COORD 200 6 ;
VARIABLE X 200 ;
X := 0 ;
COORD(1) := X|1 ;
LOOP I 2 5 ;
    X := X & I ;
    COORD(1) := COORD(1) & (X|I) ;
ENDLOOP ;
ENDPROCEDURE ;
RUN ;
END ;
"#,
        );
        assert_eq!(slot_type(&resolver, "X"), RosyType::VE());
        assert_eq!(
            slot_type(&resolver, "COORD"),
            RosyType::new(RosyBaseType::VE, 1)
        );
    }

    #[test]
    fn fox_indexed_da_array_infers_da() {
        let resolver = resolve_fox(
            r#"
BEGIN ;
PROCEDURE RUN ;
VARIABLE NM 1 ;
DAINI 2 1 0 NM ;
VARIABLE MAP1 11 6 ;
MAP1(1) := DA(1) ;
MAP1(2) := DA(1) ;
ENDPROCEDURE ;
RUN ;
END ;
"#,
        );
        assert_eq!(
            slot_type(&resolver, "MAP1"),
            RosyType::new(RosyBaseType::DA, 1)
        );
    }

    #[test]
    fn fox_coord_if_seed_then_concat_promotes_ve_array() {
        let resolver = resolve_fox(
            r#"
BEGIN ;
PROCEDURE RUN ;
VARIABLE COORD 200 6 ;
VARIABLE X 200 ;
VARIABLE I 1 ;
X := 0.01 ;
LOOP J 2 5 ;
    X := X & 0.01 ;
ENDLOOP ;
LOOP I 1 5 ;
    IF I=1 ;
        COORD(1) := (X|I) ;
    ELSEIF I>1 ;
        COORD(1) := COORD(1) & (X|I) ;
    ENDIF ;
ENDLOOP ;
ENDPROCEDURE ;
RUN ;
END ;
"#,
        );
        assert_eq!(slot_type(&resolver, "X"), RosyType::VE());
        assert_eq!(
            slot_type(&resolver, "COORD"),
            RosyType::new(RosyBaseType::VE, 1),
            "COORD was {:?}",
            slot_type(&resolver, "COORD")
        );
    }

    #[test]
    fn fox_polval_coord_pipeline_infers_ve_array() {
        let resolver = resolve_fox(
            r#"
BEGIN ;
PROCEDURE RUN ;
VARIABLE KE 1 ; VARIABLE TI 1 ; VARIABLE MASS 1 ;
VARIABLE COORD 200 6 ;
VARIABLE X 200 ; VARIABLE Y 200 ; VARIABLE Z 200 ;
VARIABLE PX 200 ; VARIABLE PY 200 ; VARIABLE PZ 200 ;
VARIABLE NP 1 ; VARIABLE P0 1 ;
NP := 100 ; MASS := 1 ; P0 := 1.8 ;
X := 0.01 ; Y := 0.01 ; Z := 0 ;
PX := 0.002 ; PY := 0.002 ; PZ := P0 ;
LOOP I 2 NP ;
    X := X & 0.01 ; Y := Y & 0.01 ; Z := Z & 0 ;
    PX := PX & 0.002 ; PY := PY & 0.002 ; PZ := PZ & P0 ;
ENDLOOP ;
LOOP I 1 NP ;
    KE := (PX|I)^2+(PY|I)^2+(PZ|I)^2 ;
    KE := KE/(SQRT(KE+MASS^2)+MASS) ;
    TI := (Z|I)*(KE+MASS)/(PZ|I) ;
    IF I=1 ;
        COORD(1) := (X|I) ; COORD(2) := (PX|I) ; COORD(3) := (Y|I) ;
        COORD(4) := (PY|I) ; COORD(5) := (TI) ; COORD(6) := (KE) ;
    ELSEIF I>1 ;
        COORD(1) := COORD(1)&(X|I) ; COORD(2) := COORD(2)&(PX|I) ;
        COORD(3) := COORD(3)&(Y|I) ; COORD(4) := COORD(4)&(PY|I) ;
        COORD(5) := COORD(5)&TI ; COORD(6) := COORD(6)&KE ;
    ENDIF ;
ENDLOOP ;
ENDPROCEDURE ;
RUN ;
END ;
"#,
        );
        assert_eq!(slot_type(&resolver, "X"), RosyType::VE());
        assert_eq!(slot_type(&resolver, "KE"), RosyType::RE());
        assert_eq!(slot_type(&resolver, "TI"), RosyType::RE());
        assert_eq!(
            slot_type(&resolver, "COORD"),
            RosyType::new(RosyBaseType::VE, 1),
            "COORD was {:?}",
            slot_type(&resolver, "COORD")
        );
    }

    #[test]
    fn fox_self_ref_indexed_da_array_stays_any() {
        let resolver = resolve_fox(
            r#"
BEGIN ;
VARIABLE NM 1 ;
DAINI 2 2 0 NM ;
VARIABLE SSL 4 4 ;
LOOP I 1 2 ;
    SSL(I) := SSL(I) + DA(1) ;
ENDLOOP ;
END ;
"#,
        );
        assert_eq!(
            slot_type(&resolver, "SSL"),
            RosyType::new(RosyBaseType::ANY, 1),
            "SSL was {:?}",
            slot_type(&resolver, "SSL")
        );
    }

    #[test]
    fn fox_unassigned_dim_array_stays_any() {
        let resolver = resolve_fox(
            r#"
BEGIN ;
VARIABLE MAP 4000 8 ;
END ;
"#,
        );
        assert_eq!(
            slot_type(&resolver, "MAP"),
            RosyType::new(RosyBaseType::ANY, 1)
        );
    }
}
