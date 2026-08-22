//! # FUNCTION Definition
//!
//! Defines a user function that returns a value. Functions capture
//! variables from their enclosing scope as closures.
//!
//! ## Syntax
//!
//! ```text
//! FUNCTION [type] name arg1 [arg2 ...];
//!     <statements>
//! ENDFUNCTION name result;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/function.rosy"))]
//! ```

use anyhow::{Context, Error, Result, anyhow, ensure};
use std::collections::BTreeSet;

use crate::{ast::*, program::statements::*, resolve::*, transpile::*};
use rosy_lib::RosyType;

/// AST node for a user-defined function declaration.
#[derive(Debug)]
pub struct FunctionStatement {
    pub name: String,
    pub args: Vec<VariableDeclarationData>,
    pub return_type: Option<RosyType>,
    pub body: Vec<Statement>,
}

impl FromRule for FunctionStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::function,
            "Expected `function` rule when building function statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();
        let (return_type, name, args) = {
            let mut start_function_inner = inner
                .next()
                .context("Missing first token `start_function`!")?
                .into_inner();

            // Return type is now optional - peek to see if the next token is a type or a name
            let first = start_function_inner
                .next()
                .context("Missing tokens in function declaration!")?;

            let (return_type, name) = if first.as_rule() == Rule::r#type {
                // we choose to ignore the dimensions of the return type for now
                //  since they can be changed dynamically
                let (return_type, _) =
                    build_type(first).context("...while building function return type")?;
                let name = start_function_inner
                    .next()
                    .context("Missing function name!")?
                    .as_str()
                    .to_string();
                (Some(return_type), name)
            } else {
                // No return type specified, first token is the function name
                let name = first.as_str().to_string();
                (None, name)
            };

            let mut args = Vec::new();
            // Collect all remaining argument names and types
            // With optional types, we peek at each token:
            //   - function_argument_name followed by type → typed argument
            //   - function_argument_name followed by another name or semicolon → untyped argument
            while let Some(arg_pair) = start_function_inner.next() {
                if arg_pair.as_rule() == Rule::semicolon {
                    break;
                }

                ensure!(
                    arg_pair.as_rule() == Rule::function_argument_name,
                    "Expected function argument name, found: {:?}",
                    arg_pair.as_rule()
                );
                let arg_name = arg_pair.as_str();

                // Peek at the next token to see if it's a type
                let next = start_function_inner.peek();
                let (argument_type, argument_dimensions) = match next {
                    Some(ref p) if p.as_rule() == Rule::r#type => {
                        let type_pair = start_function_inner.next().unwrap();
                        let (t, d) = build_type(type_pair)
                            .context("...while building function argument type")?;
                        (Some(t), d)
                    }
                    _ => (None, Vec::new()),
                };

                let argument_data = VariableDeclarationData {
                    name: arg_name.to_string(),
                    r#type: argument_type,
                    dimension_exprs: argument_dimensions,
                };
                args.push(argument_data);
            }

            (return_type, name, args)
        };

        let body = {
            let mut statements = vec![Statement {
                inner: VarDeclStatement {
                    data: VariableDeclarationData {
                        name: name.clone(),
                        r#type: return_type,
                        dimension_exprs: Vec::new(),
                    },
                }
                .into(),
                source_location: SourceLocation {
                    line: 0,
                    col: 0,
                    snippet: format!("(implicit return variable for FUNCTION {})", name),
                    file: None,
                },
            }];

            // Process remaining elements (statements and end_function)
            for element in inner {
                // Skip the end_function element
                if element.as_rule() == Rule::end_function {
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

        Ok(Some(FunctionStatement {
            name,
            args,
            return_type,
            body,
        }))
    }
}
impl TranspileableStatement for FunctionStatement {
    fn register_typeslot_declaration(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        source_location: SourceLocation,
    ) -> Option<Result<()>> {
        // Return type slot
        let ret_slot = TypeSlot::FunctionReturn(ctx.scope_path.clone(), self.name.clone());
        let fox_ret = RosyType::ANY();
        let ret_ty = self.return_type.as_ref().or_else(|| {
            crate::syntax_config::is_cosy_syntax().then_some(&fox_ret)
        });
        resolver.insert_slot(
            ret_slot.clone(),
            ret_ty,
            Some(source_location.clone()),
        );

        // Argument slots
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

        ctx.functions
            .insert(self.name.clone(), (ret_slot.clone(), arg_slots));

        // Recurse into function body with inner scope
        let mut inner_ctx = ScopeContext {
            scope_path: {
                let mut p = ctx.scope_path.clone();
                p.push(self.name.clone());
                p
            },
            // Inner scope inherits outer declarations
            variables: ctx.variables.clone(),
            functions: ctx.functions.clone(),
            procedures: ctx.procedures.clone(),
        };

        // Add args to inner scope as variable references
        for arg in &self.args {
            let arg_slot =
                TypeSlot::Argument(ctx.scope_path.clone(), self.name.clone(), arg.name.clone());
            inner_ctx.variables.insert(arg.name.clone(), arg_slot);
        }

        // The implicit return variable inside the function body
        let inner_ret_var_slot =
            TypeSlot::Variable(inner_ctx.scope_path.clone(), self.name.clone());
        // If the return type is known explicitly, the inner return var is also known
        let fox_inner = RosyType::ANY();
        let inner_ty = self.return_type.as_ref().or_else(|| {
            crate::syntax_config::is_cosy_syntax().then_some(&fox_inner)
        });
        resolver.insert_slot(
            inner_ret_var_slot.clone(),
            inner_ty,
            Some(source_location.clone()),
        );
        inner_ctx
            .variables
            .insert(self.name.clone(), inner_ret_var_slot.clone());

        if let Err(e) = resolver.discover_slots(&self.body, &mut inner_ctx) {
            return Some(Err(e));
        }

        // If the return type is NOT explicit, it depends on the inner return var
        if self.return_type.is_none() && resolver.nodes.contains_key(&inner_ret_var_slot) {
            let node = resolver.nodes.get_mut(&ret_slot).unwrap();
            node.rule = ResolutionRule::Mirror {
                source: inner_ret_var_slot.clone(),
                reason: format!(
                    "inferred from assignment to return variable '{}'",
                    self.name
                ),
            };
            node.depends_on.insert(inner_ret_var_slot);
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
        // Return type
        if self.return_type.is_none() {
            let slot = TypeSlot::FunctionReturn(current_scope.to_vec(), self.name.clone());
            if let Some(node) = resolver.nodes.get(&slot)
                && let Some(t) = &node.resolved
            {
                self.return_type = Some(*t);
            }
        }

        // Argument types
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

        // Recurse into body
        let mut inner_scope = current_scope.to_vec();
        inner_scope.push(self.name.clone());
        if let Err(e) = resolver.apply_to_ast(&mut self.body, &inner_scope) {
            return Some(Err(e));
        }

        Some(Ok(()))
    }
}
impl Transpile for FunctionStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Resolve the return type (required for transpilation)
        let resolved_return_type = self.return_type
            .ok_or_else(|| anyhow!("Type inference is not yet supported - please specify the return type for function '{}'", self.name))
            .map_err(|e| vec![e])?;

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
                        "...while resolving argument types for function '{}'",
                        self.name
                    ))),
                }
            }
            if !errors.is_empty() {
                return Err(errors);
            }
            data
        };

        let existed = context
            .functions
            .insert(
                self.name.clone(),
                TranspilationInputFunctionContext {
                    return_type: resolved_return_type,
                    args: resolved_arg_data.clone(),
                    requested_variables: BTreeSet::new(),
                },
            )
            .is_some();
        if existed && !crate::syntax_config::is_cosy_syntax() {
            return Err(vec![anyhow!(
                "Function '{}' is already defined in this scope!",
                self.name
            )]);
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
        // Function arguments may shadow parent-scope (Higher) variables —
        // see the matching comment in procedure/mod.rs. Same-scope duplicates
        // (two args with the same name) are still rejected.
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
                            "...while transpiling statement in function '{}'",
                            self.name
                        )));
                    }
                }
            }
        }

        // Update the function context with the requested variables
        if let Some(func_context) = context.functions.get_mut(&self.name) {
            func_context.requested_variables = requested_variables.clone();
        } else {
            errors.push(
                anyhow!(
                    "Function '{}' was not found in context after being inserted!",
                    self.name
                )
                .context("...while updating function context"),
            );
        }

        // Serialize arguments
        let serialized_args: Vec<String> = {
            let mut serialized_args = Vec::new();
            for var_name in requested_variables.iter() {
                if resolved_arg_data.iter().any(|a| a.name == *var_name) {
                    continue;
                }
                let Some(var_data) = inner_context.variables.get(var_name) else {
                    errors.push(
                        anyhow!(
                            "Variable '{}' was requested but not found in context!",
                            var_name
                        )
                        .context(format!("...while transpiling function '{}'", self.name)),
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

        // Serialize return type
        let serialized_return_type = resolved_return_type.as_rust_type();

        // Serialize the entire function.
        // The Rust function name is prefixed with `__fn_` to avoid shadowing
        // the implicit return variable (which uses the original Rosy name).
        // This allows recursive calls like `FIB(N-1)` to resolve to the
        // function `__fn_FIB` rather than trying to index the local `FIB: f64`.
        let rust_fn_name = format!("__fn_{}", self.name);
        let serialization = format!(
            "fn {} ( {} ) -> Result<{}> {{\n{}\n\tOk({})\n}}",
            rust_fn_name,
            serialized_args.join(", "),
            serialized_return_type,
            indent(serialized_statements.join("\n")),
            self.name
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
