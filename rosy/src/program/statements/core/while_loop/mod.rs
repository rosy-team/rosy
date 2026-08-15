//! WHILE loop statement implementation.
//!
//! ## Syntax
//! ```text
//! WHILE <condition>; <statements> ENDWHILE;
//! ```
//!
//! The condition is evaluated before each iteration. If it evaluates to TRUE,
//! the body is executed and then the condition is checked again. When the
//! condition evaluates to FALSE, execution continues after ENDWHILE.
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
    program::{
        expressions::Expr,
        statements::{SourceLocation, Statement},
    },
    resolve::{ScopeContext, TypeResolver},
    transpile::*,
};
use rosy_lib::RosyType;

#[derive(Debug)]
pub struct WhileStatement {
    pub condition: Expr,
    pub body: Vec<Statement>,
}

impl FromRule for WhileStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::while_loop,
            "Expected `while_loop` rule when building while statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        // Parse start_while to get condition
        let condition = {
            let mut start_while_inner = inner
                .next()
                .context("Missing first token `start_while`!")?
                .into_inner();

            let condition_pair = start_while_inner
                .next()
                .context("Missing condition expression in WHILE statement!")?;
            Expr::from_rule(condition_pair)
                .context("Failed to build condition expression in WHILE statement!")?
                .ok_or_else(|| {
                    anyhow::anyhow!("Expected expression for condition in WHILE statement")
                })?
        };

        let mut body = Vec::new();
        // Process remaining elements (statements and end)
        for element in inner {
            // Skip the end element
            if element.as_rule() == Rule::end_while {
                break;
            }

            let pair_input = element.as_str();
            if let Some(stmt) = Statement::from_rule(element)
                .with_context(|| format!("Failed to build statement from:\n{}", pair_input))?
            {
                body.push(stmt);
            }
        }

        Ok(Some(WhileStatement { condition, body }))
    }
}
impl TranspileableStatement for WhileStatement {
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
            result: resolver.discover_slots(&self.body, &mut ctx.clone()),
        }
    }
    fn hydrate_resolved_types(
        &mut self,
        resolver: &TypeResolver,
        current_scope: &[String],
    ) -> TypeHydrationResult {
        if let Err(e) = resolver.apply_to_ast(&mut self.body, current_scope) {
            return TypeHydrationResult::Hydrated { result: Err(e) };
        }
        TypeHydrationResult::Hydrated { result: Ok(()) }
    }
}
impl Transpile for WhileStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Verify the condition is a logical expression
        let condition_type = self.condition.type_of(context).map_err(|e| vec![e])?;
        if condition_type != RosyType::LO() {
            return Err(vec![anyhow!(
                "WHILE condition must be of type 'LO' (logical), found '{}'",
                condition_type
            )]);
        }

        let mut inner_context: TranspilationInputContext = context.clone();
        inner_context.in_loop = true;
        let mut requested_variables = BTreeSet::new();
        let mut serialized_statements = Vec::new();
        let mut errors = Vec::new();

        // Transpile each inner statement
        for stmt in &self.body {
            match stmt.transpile(&mut inner_context) {
                Ok(output) => {
                    serialized_statements.push(output.serialization);
                    requested_variables.extend(output.requested_variables);
                }
                Err(stmt_errors) => {
                    for e in stmt_errors {
                        errors.push(e.context("...while transpiling statement in WHILE loop"));
                    }
                }
            }
        }

        // Serialize the condition expression
        let cond_output = match self.condition.transpile(context) {
            Ok(output) => output,
            Err(vec_err) => {
                for e in vec_err {
                    errors.push(e.context("...while transpiling condition for WHILE loop"));
                }
                return Err(errors);
            }
        };
        requested_variables.extend(cond_output.requested_variables.iter().cloned());

        // Generate Rust while loop
        // LO (bool) is Copy, so as_value() gives the plain bool or (*&X)
        let serialization = format!(
            "while {} {{\n{}\n}}",
            cond_output.as_value(),
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
