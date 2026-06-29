//! # PWTIME Statement
//!
//! Returns the elapsed wall-clock time in seconds since program start.
//!
//! ## Syntax
//!
//! ```text
//! PWTIME v;
//! ```
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

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{
        expressions::core::variable_identifier::VariableIdentifier, statements::SourceLocation,
    },
    resolve::{ScopeContext, TypeResolver},
    transpile::{
        InferenceEdgeResult, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableStatement, TypeHydrationResult, TypeslotDeclarationResult, VariableScope,
        add_context_to_all,
    },
};

#[derive(Debug)]
pub struct PwtimeStatement {
    pub identifier: VariableIdentifier,
}

impl FromRule for PwtimeStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::pwtime,
            "Expected `pwtime` rule when building PWTIME statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let expr_pair = inner
            .next()
            .context("Missing variable expression in PWTIME!")?;
        let identifier = VariableIdentifier::from_rule(expr_pair)
            .context("Failed to build variable identifier in PWTIME")?
            .ok_or_else(|| anyhow::anyhow!("Expected variable identifier in PWTIME"))?;

        Ok(Some(PwtimeStatement { identifier }))
    }
}

impl TranspileableStatement for PwtimeStatement {
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

impl Transpile for PwtimeStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let output = self.identifier.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling identifier in PWTIME".to_string())
        })?;
        requested_variables.extend(output.requested_variables.clone());

        let dereference = match context
            .variables
            .get(&self.identifier.name)
            .ok_or_else(|| {
                vec![anyhow::anyhow!(
                    "Variable '{}' is not defined in this scope!",
                    self.identifier.name
                )]
            })?
            .scope
        {
            VariableScope::Local => "",
            VariableScope::Arg => "*",
            VariableScope::Higher => {
                requested_variables.insert(self.identifier.name.clone());
                "*"
            }
        };

        // Use the timer helper from the output template so PWTIME works in
        // generated function/procedure bodies as well as the main wrapper.
        let serialization = format!(
            "{}{} = rosy_elapsed_seconds();",
            dereference, output.serialization
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
