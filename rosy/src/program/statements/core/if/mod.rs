//! # IF Statement (Conditional Branching)
//!
//! Conditional execution with optional `ELSEIF` and `ELSE` clauses.
//!
//! ## Syntax
//!
//! ```text
//! IF condition;
//!     <statements>
//! [ELSEIF condition;
//!     <statements>]
//! [ELSE;
//!     <statements>]
//! ENDIF;
//! ```
//!
//! The condition must evaluate to a `LO` (logical/boolean) type.
//!
//! > **COSY note:** COSY INFINITY does not have an `ELSE` keyword.
//! > The idiomatic equivalent is `ELSEIF LO(1);` (always-true guard).
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

use anyhow::{Context, Error, Result, anyhow, bail, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{
        expressions::Expr,
        statements::{SourceLocation, Statement},
    },
    resolve::*,
    transpile::*,
};
use rosy_lib::RosyType;

/// AST node for the `IF ... [ELSEIF ...] [ELSE] ENDIF;` statement.
#[derive(Debug)]
pub struct IfStatement {
    pub condition: Expr,
    pub then_body: Vec<Statement>,
    pub elseif_clauses: Vec<ElseIfClause>,
    pub else_body: Option<Vec<Statement>>,
}
#[derive(Debug)]
pub struct ElseIfClause {
    pub condition: Expr,
    pub body: Vec<Statement>,
}

impl FromRule for IfStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::if_statement,
            "Expected `if_statement` rule when building if statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        // Parse the main IF clause
        let (condition, then_body) = {
            let mut if_clause_inner = inner.next().context("Missing if_clause!")?.into_inner();

            let condition = Expr::from_rule(
                if_clause_inner
                    .next()
                    .context("Missing condition in IF clause!")?,
            )
            .context("Failed to build IF condition expression!")?
            .ok_or_else(|| anyhow::anyhow!("Expected expression for IF condition"))?;

            let mut then_body = Vec::new();
            for stmt_pair in if_clause_inner {
                if stmt_pair.as_rule() == Rule::semicolon {
                    continue;
                }

                let pair_input = stmt_pair.as_str();
                if let Some(stmt) = Statement::from_rule(stmt_pair).with_context(|| {
                    format!("Failed to build statement in IF body from:\n{}", pair_input)
                })? {
                    then_body.push(stmt);
                }
            }

            (condition, then_body)
        };

        // Parse ELSEIF clauses
        let mut elseif_clauses = Vec::new();
        let mut else_body = None;
        for element in inner {
            match element.as_rule() {
                Rule::elseif_clause => {
                    let mut elseif_inner = element.into_inner();

                    let condition = Expr::from_rule(
                        elseif_inner
                            .next()
                            .context("Missing condition in ELSEIF clause!")?,
                    )
                    .context("Failed to build ELSEIF condition expression!")?
                    .ok_or_else(|| anyhow::anyhow!("Expected expression for ELSEIF condition"))?;

                    let mut body = Vec::new();
                    for stmt_pair in elseif_inner {
                        if stmt_pair.as_rule() == Rule::semicolon {
                            continue;
                        }

                        let pair_input = stmt_pair.as_str();
                        if let Some(stmt) = Statement::from_rule(stmt_pair).with_context(|| {
                            format!(
                                "Failed to build statement in ELSEIF body from:\n{}",
                                pair_input
                            )
                        })? {
                            body.push(stmt);
                        }
                    }

                    elseif_clauses.push(ElseIfClause { condition, body });
                }
                Rule::else_clause => {
                    let else_inner = element.into_inner();
                    let mut body = Vec::new();
                    for stmt_pair in else_inner {
                        if stmt_pair.as_rule() == Rule::semicolon {
                            continue;
                        }

                        let pair_input = stmt_pair.as_str();
                        if let Some(stmt) = Statement::from_rule(stmt_pair).with_context(|| {
                            format!(
                                "Failed to build statement in ELSE body from:\n{}",
                                pair_input
                            )
                        })? {
                            body.push(stmt);
                        }
                    }
                    else_body = Some(body);
                }
                Rule::endif => {
                    // End of IF statement
                    break;
                }
                _ => {
                    bail!(
                        "Unexpected element in IF statement: {:?}",
                        element.as_rule()
                    );
                }
            }
        }

        Ok(Some(IfStatement {
            condition,
            then_body,
            elseif_clauses,
            else_body,
        }))
    }
}
impl TranspileableStatement for IfStatement {
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
        if let Err(e) = resolver.discover_slots(&self.then_body, &mut ctx.clone()) {
            return InferenceEdgeResult::HasEdges { result: Err(e) };
        }
        for elseif in &self.elseif_clauses {
            if let Err(e) = resolver.discover_slots(&elseif.body, &mut ctx.clone()) {
                return InferenceEdgeResult::HasEdges { result: Err(e) };
            }
        }
        if let Some(else_body) = &self.else_body
            && let Err(e) = resolver.discover_slots(else_body, &mut ctx.clone())
        {
                return InferenceEdgeResult::HasEdges { result: Err(e) };
        }
        InferenceEdgeResult::HasEdges { result: Ok(()) }
    }
    fn hydrate_resolved_types(
        &mut self,
        resolver: &TypeResolver,
        current_scope: &[String],
    ) -> TypeHydrationResult {
        if let Err(e) = resolver.apply_to_ast(&mut self.then_body, current_scope) {
            return TypeHydrationResult::Hydrated { result: Err(e) };
        }
        for elseif in &mut self.elseif_clauses {
            if let Err(e) = resolver.apply_to_ast(&mut elseif.body, current_scope) {
                return TypeHydrationResult::Hydrated { result: Err(e) };
            }
        }
        if let Some(else_body) = &mut self.else_body
            && let Err(e) = resolver.apply_to_ast(else_body, current_scope)
        {
                return TypeHydrationResult::Hydrated { result: Err(e) };
        }
        TypeHydrationResult::Hydrated { result: Ok(()) }
    }
}
impl Transpile for ElseIfClause {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Verify the condition is a logical expression
        let condition_type = self
            .condition
            .type_of(context)
            .context("...while determining type of ELSEIF condition expression")
            .map_err(|e| vec![e])?;
        if condition_type != RosyType::LO() {
            return Err(vec![anyhow!(
                "ELSEIF condition must be of type 'LO' (logical), found '{condition_type}'"
            )]);
        }

        let mut requested_variables = BTreeSet::new();
        let mut errors = Vec::new();

        // Transpile the condition
        let cond_output = match self.condition.transpile(context) {
            Ok(output) => output,
            Err(err_vec) => {
                for err in err_vec {
                    errors.push(err.context("...while transpiling ELSEIF condition expression"));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(cond_output.requested_variables.iter().cloned());

        // Transpile the body
        let serialized_statements: Vec<String> = {
            let mut serialized_statements = Vec::new();
            let mut inner_context: TranspilationInputContext = context.clone();

            // Transpile each inner statement
            for stmt in &self.body {
                match stmt.transpile(&mut inner_context) {
                    Ok(output) => {
                        serialized_statements.push(output.serialization);
                        requested_variables.extend(output.requested_variables);
                    }
                    Err(stmt_errors) => {
                        for e in stmt_errors {
                            errors.push(e.context("...while transpiling statement in ELSEIF body"));
                        }
                    }
                }
            }
            serialized_statements
        };

        let serialization = format!(
            "else if {} {{\n{}\n}}",
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
impl Transpile for IfStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Verify the condition is a logical expression
        let condition_type = self
            .condition
            .type_of(context)
            .context("...while determining type of IF condition expression")
            .map_err(|e| vec![e])?;
        if condition_type != RosyType::LO() {
            return Err(vec![anyhow!(
                "IF condition must be of type 'LO' (logical), found '{condition_type}'"
            )]);
        }

        let mut requested_variables = BTreeSet::new();
        let mut errors = Vec::new();

        // Transpile the condition
        let cond_output = match self.condition.transpile(context) {
            Ok(output) => output,
            Err(err_vec) => {
                for err in err_vec {
                    errors.push(err.context("...while transpiling IF condition expression"));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(cond_output.requested_variables.iter().cloned());

        // Transpile the primary if clause body
        let serialized_if_statements: Vec<String> = {
            let mut serialized_if_statements = Vec::new();
            let mut inner_context: TranspilationInputContext = context.clone();

            // Transpile each inner statement
            for stmt in &self.then_body {
                match stmt.transpile(&mut inner_context) {
                    Ok(output) => {
                        serialized_if_statements.push(output.serialization);
                        requested_variables.extend(output.requested_variables);
                    }
                    Err(err_vec) => {
                        for err in err_vec {
                            errors.push(err.context("...while transpiling statement in IF body"));
                        }
                    }
                }
            }

            serialized_if_statements
        };

        // Transpile each ELSEIF clause
        let serialized_elseif_clauses = {
            let mut serialized_elseif_clauses = Vec::new();
            for elseif_clause in &self.elseif_clauses {
                match elseif_clause.transpile(context) {
                    Ok(output) => {
                        requested_variables.extend(output.requested_variables);
                        serialized_elseif_clauses.push(output.serialization);
                    }
                    Err(vec_err) => {
                        for err in vec_err {
                            errors.push(err.context("...while transpiling ELSEIF clause"));
                        }
                    }
                }
            }
            serialized_elseif_clauses
        };

        // Transpile the ELSE clause body, if it exists
        let serialized_else_clause = if let Some(else_body) = &self.else_body {
            let mut serialized_else_statements = Vec::new();
            let mut inner_context: TranspilationInputContext = context.clone();

            // Transpile each inner statement
            for stmt in else_body {
                match stmt.transpile(&mut inner_context) {
                    Ok(output) => {
                        serialized_else_statements.push(output.serialization);
                        requested_variables.extend(output.requested_variables);
                    }
                    Err(stmt_errors) => {
                        for e in stmt_errors {
                            errors.push(e.context("...while transpiling statement in ELSE body"));
                        }
                    }
                }
            }
            format!(
                " else {{\n{}\n}}",
                indent(serialized_else_statements.join("\n"))
            )
        } else {
            String::new()
        };

        let serialization = format!(
            "if {} {{\n{}\n}}{}{}",
            cond_output.as_value(),
            indent(serialized_if_statements.join("\n")),
            if serialized_elseif_clauses.is_empty() {
                String::new()
            } else {
                format!(" {}", serialized_elseif_clauses.join(" "))
            },
            serialized_else_clause
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
impl TranspileableStatement for ElseIfClause {
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
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> InferenceEdgeResult {
        InferenceEdgeResult::NoEdges
    }
    fn hydrate_resolved_types(
        &mut self,
        _resolver: &TypeResolver,
        _current_scope: &[String],
    ) -> TypeHydrationResult {
        TypeHydrationResult::NothingToHydrate
    }
}
