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
                )),);
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
                if let Ok(new_type) = resolver.evaluate_recipe(&recipe)
                    && new_type != explicit_type
                {
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
                        return Some(Err(RosyError::at(source_location.clone(), msg).into()),);
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

                // If the old recipe can't be evaluated yet (e.g. X:=10.5*J
                // where J has a known recipe but isn't resolved), try
                // temporarily resolving leaf dependencies so the conflict
                // checker can detect RE↔VE coercion patterns.
                let mut temp_leaf_slots: Vec<TypeSlot> = Vec::new();
                if old_type_result.is_err() {
                    let all_deps: HashSet<TypeSlot> = {
                        let mut d = resolver
                            .nodes
                            .get(&var_slot)
                            .map(|n| n.depends_on.clone())
                            .unwrap_or_default();
                        d.extend(deps.iter().cloned());
                        d
                    };

                    for dep_slot in &all_deps {
                        if *dep_slot == var_slot {
                            continue;
                        }
                        if let Some(dep_node) = resolver.nodes.get(dep_slot)
                            && dep_node.resolved.is_none()
                            && dep_node.depends_on.is_empty()
                            && let ResolutionRule::InferredFrom { recipe: ref r, .. } =
                                dep_node.rule
                            && let Ok(t) = resolver.evaluate_recipe(r)
                        {
                            temp_leaf_slots.push(dep_slot.clone());
                            resolver.nodes.get_mut(dep_slot).unwrap().resolved = Some(t);
                        }
                    }

                    if !temp_leaf_slots.is_empty() {
                        old_type_result = resolver.evaluate_recipe(old_recipe);
                        new_type_result = resolver.evaluate_recipe(&dimensioned_recipe);
                    }
                }

                // If the new recipe failed but old succeeded, try again
                // with a temporary assumption that the variable has the
                // old type (handles self-referential patterns like Y:=Y&I)
                if new_type_result.is_err()
                    && let Ok(ref old_type) = old_type_result
                {
                        // Temporarily mark this slot as resolved
                        if let Some(node) = resolver.nodes.get_mut(&var_slot) {
                            node.resolved = Some(*old_type);
                        }
                        new_type_result = resolver.evaluate_recipe(&dimensioned_recipe);
                        // Undo the temporary resolution
                        if let Some(node) = resolver.nodes.get_mut(&var_slot) {
                            node.resolved = None;
                        }
                }

                // Undo temporary leaf resolutions
                for slot in &temp_leaf_slots {
                    if let Some(node) = resolver.nodes.get_mut(slot) {
                        node.resolved = None;
                    }
                }

                if let (Ok(old_type), Ok(new_type)) = (old_type_result, new_type_result)
                    && old_type != new_type
                {
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
                        return Some(Err(RosyError::at(source_location.clone(), msg).into()),);
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
        if variable_type != value_type {
            return Err(vec![anyhow!(
                "Cannot assign value of type '{}' to variable '{}' of type '{}'!",
                value_type,
                self.identifier.name,
                variable_type
            )]);
        }

        // Optimization: detect `X := X & expr` and generate in-place append
        if self.identifier.num_index_dimensions() == 0
            && let Some(result) = value
                .inner
                .try_inplace_append(&self.identifier.name, context)
        {
            return result;
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

        let serialized_value = value_output.as_owned(&variable_type);

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

        let num_indices = self.identifier.num_index_dimensions();
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
            let mut_ref = match var_scope {
                VariableScope::Local => format!("&mut {}", self.identifier.name),
                VariableScope::Arg | VariableScope::Higher => self.identifier.name.clone(),
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
                    "{{ let __rosy_self_ref_tmp = {value}; *{lhs} = __rosy_self_ref_tmp; }}",
                    value = serialized_value,
                    lhs = result,
                )
            } else {
                format!("*{} = {};", result, serialized_value)
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
