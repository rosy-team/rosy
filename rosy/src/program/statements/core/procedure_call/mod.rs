//! # Procedure Call Statement
//!
//! Invokes a user-defined procedure with arguments.
//!
//! ## Syntax
//!
//! ```text
//! PROCNAME arg1 [arg2 ...];
//! ```
//!
//! ## Note
//!
//! Arguments are passed by mutable reference. The procedure may modify
//! the caller's variables.
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

use anyhow::{Context, Error, Result, anyhow, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{expressions::Expr, statements::SourceLocation},
    resolve::*,
    transpile::*,
};

/// AST node for a procedure call statement.
#[derive(Debug)]
pub struct ProcedureCallStatement {
    pub name: String,
    pub args: Vec<Expr>,
}

impl FromRule for ProcedureCallStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::procedure_call,
            "Expected `procedure_call` rule when building procedure call statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();
        let name = inner
            .next()
            .context("Missing procedure name in procedure call!")?
            .as_str()
            .to_string();

        let mut args = Vec::new();
        // Collect all remaining arguments (expressions)
        for arg_pair in inner {
            if arg_pair.as_rule() == Rule::semicolon {
                break;
            }

            let expr = Expr::from_rule(arg_pair)
                .context("Failed to build expression in procedure call!")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression in procedure call"))?;
            args.push(expr);
        }

        Ok(Some(ProcedureCallStatement { name, args }))
    }
}
impl TranspileableStatement for ProcedureCallStatement {
    fn register_typeslot_declaration(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> TypeslotDeclarationResult {
        TypeslotDeclarationResult::NotAVarFuncOrProcedureDecl
    }
    fn wire_inference_edges(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> InferenceEdgeResult {
        InferenceEdgeResult::HasEdges {
            result: resolver.discover_call_site_deps(&self.name, &self.args, false, ctx),
        }
    }
    fn hydrate_resolved_types(
        &mut self,
        _resolver: &TypeResolver,
        _current_scope: &[String],
    ) -> TypeHydrationResult {
        TypeHydrationResult::NothingToHydrate
    }
}
impl Transpile for ProcedureCallStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Start by checking that the procedure exists
        let proc_context = match context.procedures.get(&self.name) {
            Some(ctx) => ctx,
            None => {
                let hint = context.procedure_hint(&self.name);
                return Err(vec![anyhow!(
                    "procedure '{}' is not defined in this scope!{}",
                    self.name,
                    hint
                )]);
            }
        }
        .clone();

        // Check that the number of arguments is correct
        if proc_context.args.len() != self.args.len() {
            return Err(vec![anyhow!(
                "procedure '{}' expects {} arguments, but {} were provided!",
                self.name,
                proc_context.args.len(),
                self.args.len()
            )]);
        }
        let mut errors = Vec::new();
        let mut requested_variables = BTreeSet::new();
        let mut serialized_args = Vec::new();
        // Serialize the requested variables from the procedure context
        for var in &proc_context.requested_variables {
            // rosy_mpi_context is the same `&mut RosyMPIContext` shape at
            // top-level (via the template's indirection) and inside procedure
            // bodies (as a parameter). Pass the binding bare and let Rust
            // auto-reborrow; the surrounding scope's request propagation
            // happens via line 281's `requested_variables.extend(...)`.
            if var == "rosy_mpi_context" {
                serialized_args.push("rosy_mpi_context".to_string());
                continue;
            }

            let var_data = context.variables.get(var).ok_or(vec![anyhow!(
                "Could not find variable '{}' requested by procedure '{}'",
                var,
                self.name
            )])?;

            let serialized_arg = match var_data.scope {
                VariableScope::Higher => var.to_string(),
                VariableScope::Arg => var.to_string(),
                VariableScope::Local => format!("&mut {}", var),
            };
            serialized_args.push(serialized_arg);
        }

        // See expressions/core/var_expr/mod.rs for the symmetric design rationale.
        // Two call-site borrow hazards lower to the same fix — pre-evaluate
        // offending args into fresh local temps:
        //   (a) bare-variable duplicates (E0499: two `&mut <var>`)
        //   (b) mixed mut/shared in one arg list (E0502: `&mut *X` somewhere
        //       overlapping a `&*X` baked into another arg's expression)
        let mut first_occurrence: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut prelude_decls: Vec<String> = Vec::new();
        let mut prelude_overrides: std::collections::HashMap<usize, String> =
            std::collections::HashMap::new();
        // Writebacks — one entry per duplicated bare-variable arg. After the
        // call, copy each clone temp back into its source variable so the
        // procedure's mutation lands in the caller's binding (matching COSY's
        // pass-by-reference convention where later args are typically
        // outputs — e.g. `ANM N M O` called as `ANM A B B` writes the result
        // into B). Without this, dup-arg clones silently swallow the writes.
        let mut writeback_decls: Vec<String> = Vec::new();

        // Pass 1 — record (a) bare-variable duplicates.
        for (i, arg_expr) in self.args.iter().enumerate() {
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
        for (i, arg_expr) in self.args.iter().enumerate() {
            match arg_expr.transpile(context) {
                Ok(arg_output) => {
                    // Check the type is correct
                    let provided_type = arg_expr.type_of(context).map_err(|e| vec![e])?;
                    let expected_type = proc_context
                        .args
                        .get(i)
                        .ok_or(vec![anyhow!(
                            "procedure '{}' expects {} arguments, but {} were provided!",
                            self.name,
                            proc_context.args.len(),
                            self.args.len()
                        )])?
                        .r#type;
                    if provided_type != expected_type {
                        errors.push(anyhow!(
                            "procedure '{}' expects argument {} ('{}') to be of type '{}', but type '{}' was provided!",
                            self.name, i+1, proc_context.args[i].name, expected_type, provided_type
                        ));
                    } else if let Some(override_serialization) = prelude_overrides.remove(&i) {
                        // (a) bare-variable duplicate.
                        serialized_args.push(override_serialization);
                        requested_variables.extend(arg_output.requested_variables);
                    } else if arg_expr.as_bare_variable_name().is_some() {
                        // Bare-variable first-occurrence — use as_mut_ref()
                        // so a Ref-kind arg (already `&mut T`) becomes
                        // `&mut *X` (a fresh reborrow) instead of
                        // `&mut <&mut T>`.
                        serialized_args.push(arg_output.as_mut_ref());
                        requested_variables.extend(arg_output.requested_variables);
                    } else {
                        // (b) Non-bare arg: pre-evaluate so any borrows the
                        // expression takes are released before the call.
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
                            "...while transpiling argument {} for procedure '{}'",
                            i + 1,
                            self.name
                        )));
                    }
                }
            }
        }

        // Serialize the entire procedure (wrap in a block when any prelude
        // temps are needed so they're scoped to this single call). Writebacks
        // run *after* the call to copy duplicate-arg clones back into their
        // source variables (see writeback_decls comment above).
        let call = format!(
            "{}({}).context(\"...while calling procedure '{}'\")?;",
            self.name,
            serialized_args.join(", "),
            self.name
        );
        let serialization = if prelude_decls.is_empty() && writeback_decls.is_empty() {
            call
        } else {
            format!(
                "{{ {} {} {} }}",
                prelude_decls.join(" "),
                call,
                writeback_decls.join(" ")
            )
        };
        if errors.is_empty() {
            // Transitive global capture: see matching comment in
            // expressions/core/var_expr/mod.rs. The procedure's own
            // captured globals must propagate up to the caller's signature,
            // otherwise nested calls fail to resolve their globals.
            requested_variables.extend(proc_context.requested_variables.iter().cloned());
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
