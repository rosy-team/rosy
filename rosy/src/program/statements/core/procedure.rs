//! # PROCEDURE Definition
//!
//! Defines a user procedure (subroutine with no return value).
//! Procedures capture variables from their enclosing scope as closures
//! and can modify them via mutable references.
//!
//! ## Syntax
//!
//! ```text
//! PROCEDURE name [arg1 arg2 ...];
//!     <statements>
//! ENDPROCEDURE;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/procedure.rosy"))]
//! ```

use anyhow::{Context, Error, Result, anyhow, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::statements::*,
    resolve::{ScopeContext, TypeResolver, TypeSlot},
    transpile::*,
};
use rosy_lib::RosyType;

/// AST node for a user-defined procedure declaration.
#[derive(Debug)]
pub struct ProcedureStatement {
    pub name: String,
    pub args: Vec<VariableDeclarationData>,
    pub body: Vec<Statement>,
}

impl FromRule for ProcedureStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::procedure,
            "Expected `procedure` rule when building procedure statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();
        let (name, args) = {
            let mut start_procedure_inner = inner
                .next()
                .context("Missing first token `start_procedure`!")?
                .into_inner();

            let name = start_procedure_inner
                .next()
                .context("Missing procedure name!")?
                .as_str()
                .to_string();

            let mut args = Vec::new();
            // Collect all remaining argument names and types
            for arg_pair in start_procedure_inner {
                if arg_pair.as_rule() == Rule::semicolon {
                    break;
                }
                ensure!(
                    arg_pair.as_rule() == Rule::procedure_argument_name_and_type,
                    "Expected procedure argument name and type, found: {:?}",
                    arg_pair.as_rule()
                );

                let mut arg_inner = arg_pair.into_inner();
                let name = arg_inner
                    .next()
                    .context("Missing procedure argument name!")?
                    .as_str();

                // Type is now optional
                let (r#type, dimension_exprs) = if let Some(type_pair) = arg_inner.next() {
                    let (t, d) = build_type(type_pair)
                        .context("...while building procedure argument type")?;
                    (Some(t), d)
                } else {
                    (None, Vec::new())
                };

                let variable_data = VariableDeclarationData {
                    name: name.to_string(),
                    r#type,
                    dimension_exprs,
                };
                args.push(variable_data);
            }

            (name, args)
        };

        let body = {
            let mut statements = Vec::new();

            // Process remaining elements (statements and end_procedure)
            for element in inner {
                // Skip the end_procedure element
                if element.as_rule() == Rule::end_procedure {
                    break;
                }

                let pair_input = element.as_str();
                if let Some(stmt) = Statement::from_rule(element)
                    .with_context(|| format!("Failed to build statement from:\n{}", pair_input))?
                {
                    statements.push(stmt);
                }
            }

            statements
        };

        Ok(Some(ProcedureStatement { name, args, body }))
    }
}
impl TranspileableStatement for ProcedureStatement {
    fn register_typeslot_declaration(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        source_location: SourceLocation,
    ) -> Option<Result<()>> {
        let mut arg_slots = Vec::new();
        for arg in &self.args {
            let arg_slot =
                TypeSlot::Argument(ctx.scope_path.clone(), self.name.clone(), arg.name.clone());
            let fox_any = RosyType::ANY();
            let ty = arg.r#type.as_ref().or_else(|| {
                crate::syntax_config::is_cosy_syntax().then_some(&fox_any)
            });
            resolver.insert_slot(
                arg_slot.clone(),
                ty,
                Some(source_location.clone()),
            );
            arg_slots.push((arg.name.clone(), arg_slot));
        }

        ctx.procedures.insert(self.name.clone(), arg_slots);
        // Recurse into procedure body
        let mut inner_ctx = ScopeContext {
            scope_path: {
                let mut p = ctx.scope_path.clone();
                p.push(self.name.clone());
                p
            },
            variables: ctx.variables.clone(),
            functions: ctx.functions.clone(),
            procedures: ctx.procedures.clone(),
        };

        for arg in &self.args {
            let arg_slot =
                TypeSlot::Argument(ctx.scope_path.clone(), self.name.clone(), arg.name.clone());
            inner_ctx.variables.insert(arg.name.clone(), arg_slot);
        }

        if let Err(e) = resolver.discover_slots(&self.body, &mut inner_ctx) {
            return Some(Err(e));
        }

        Some(Ok(()))
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
        resolver: &TypeResolver,
        current_scope: &[String],
    ) -> Option<Result<()>> {
        for arg in &mut self.args {
            if arg.r#type.is_none() {
                let slot =
                    TypeSlot::Argument(current_scope.to_vec(), self.name.clone(), arg.name.clone());
                if let Some(node) = resolver.nodes.get(&slot)
                    && let Some(t) = &node.resolved
                {
                    arg.r#type = Some(*t);
                }
            }
        }

        let mut inner_scope = current_scope.to_vec();
        inner_scope.push(self.name.clone());
        if let Err(e) = resolver.apply_to_ast(&mut self.body, &inner_scope) {
            return Some(Err(e));
        }

        Some(Ok(()))
    }
}
impl Transpile for ProcedureStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Resolve all argument types (required for transpilation)
        let resolved_arg_data: Vec<VariableData> = {
            let mut data = Vec::new();
            let mut errors = Vec::new();
            for arg in &self.args {
                match arg.require_type() {
                    Ok(t) => data.push(VariableData {
                        name: arg.name.clone(),
                        r#type: t,
                    }),
                    Err(e) => errors.push(e.context(format!(
                        "...while resolving argument types for procedure '{}'",
                        self.name
                    ))),
                }
            }
            if !errors.is_empty() {
                return Err(errors);
            }
            data
        };

        // Insert the procedure signature, but check it doesn't already exist
        if context.functions.contains_key(&self.name)
            || context
                .procedures
                .insert(
                    self.name.clone(),
                    TranspilationInputProcedureContext {
                        args: resolved_arg_data.clone(),
                        requested_variables: BTreeSet::new(),
                        requested_types: Default::default(),
                    },
                )
                .is_some()
        {
            if !crate::syntax_config::is_cosy_syntax() {
                return Err(vec![anyhow!(
                    "Procedure '{}' is already defined in this scope!",
                    self.name
                )]);
            }
        }

        // Define and raise the level of any existing variables
        let mut inner_context: TranspilationInputContext = context.clone();
        inner_context.in_loop = false;
        let mut requested_variables = BTreeSet::new();
        let mut serialized_statements = Vec::new();
        let mut errors = Vec::new();
        for ScopedVariableData { scope, .. } in inner_context.variables.values_mut() {
            *scope = match *scope {
                VariableScope::Local => VariableScope::Higher,
                VariableScope::Arg => VariableScope::Higher,
                VariableScope::Higher => VariableScope::Higher,
            }
        }
        // A procedure argument is allowed to shadow a parent-scope (Higher)
        // variable — including any global. The previous logic erroneously
        // rejected `PROCEDURE WSET W ;` because `W` is a global VARIABLE,
        // even though shadowing is well-defined in lexically scoped languages
        // (and was already permitted for VARIABLE declarations by the var_decl
        // shadowing fix). A genuine duplicate — two args with the same name —
        // is still a real conflict because the second insert finds an `Arg`
        // (not `Higher`) entry already in the local scope.
        for arg_data in &resolved_arg_data {
            let previous = inner_context.variables.insert(
                arg_data.name.clone(),
                ScopedVariableData {
                    scope: VariableScope::Arg,
                    data: arg_data.clone(),
                },
            );
            if let Some(prev) = previous
                && prev.scope != VariableScope::Higher
            {
                errors.push(anyhow!(
                    "Argument '{}' is already defined in this scope!",
                    arg_data.name
                ));
            }
        }

        // Transpile each inner statement
        for stmt in &self.body {
            match stmt.transpile(&mut inner_context) {
                Ok(output) => {
                    serialized_statements.push(output.serialization);
                    requested_variables.extend(output.requested_variables);
                }
                Err(stmt_errors) => {
                    for e in stmt_errors {
                        errors.push(e.context(format!(
                            "...while transpiling statement in procedure '{}'",
                            self.name
                        )));
                    }
                }
            }
        }

        // Update the procedure context with the requested variables,
        //  first removing those which are locally defined or args
        requested_variables.retain(|var| {
            if let Some(var_data) = inner_context.variables.get(var) {
                !matches!(var_data.scope, VariableScope::Local | VariableScope::Arg)
            } else {
                true
            }
        });
        if let Some(proc_context) = context.procedures.get_mut(&self.name) {
            proc_context.requested_variables = requested_variables.clone();
            proc_context.requested_types = requested_variables
                .iter()
                .filter_map(|n| {
                    inner_context
                        .variables
                        .get(n)
                        .map(|v| (n.clone(), v.data.r#type))
                })
                .collect();
        } else {
            errors.push(
                anyhow!(
                    "Procedure '{}' was not found in context after being inserted!",
                    self.name
                )
                .context("...while updating procedure context"),
            );
        }

        // Serialize arguments
        let serialized_args: Vec<String> = {
            let mut serialized_args = Vec::new();
            for var_name in requested_variables.iter() {
                if resolved_arg_data.iter().any(|a| a.name == *var_name) {
                    continue;
                }
                // rosy_mpi_context is a transpiler-injected runtime singleton
                // (used by PLOOP, see ploop/mod.rs). It threads through the
                // procedure call chain as a typed reference so PLOOP works
                // inside nested procedures, not just at top-level.
                if var_name == "rosy_mpi_context" {
                    serialized_args.push("rosy_mpi_context: &mut RosyMPIContext".to_string());
                    continue;
                }

                let Some(var_data) = inner_context.variables.get(var_name) else {
                    errors.push(
                        anyhow!(
                            "Variable '{}' was requested but not found in context!",
                            var_name
                        )
                        .context(format!("...while transpiling procedure '{}'", self.name)),
                    );
                    continue;
                };

                serialized_args.push(format!(
                    "{}: &mut {}",
                    var_name,
                    var_data.data.r#type.as_rust_type()
                ));
            }
            for arg_data in &resolved_arg_data {
                serialized_args.push(format!(
                    "{}: &mut {}",
                    arg_data.name,
                    arg_data.r#type.as_rust_type()
                ));
            }
            serialized_args
        };

        let serialization = format!(
            "fn {} ( {} ) -> Result<()> {{\n{}\n\n\tOk(())\n}}",
            if crate::syntax_config::is_cosy_syntax() {
                format!("__proc_{}", self.name)
            } else {
                self.name.clone()
            },
            serialized_args.join(", "),
            indent(serialized_statements.join("\n"))
        );
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
