//! # DAFILT Statement (DA Filter)
//!
//! Filters a DA or CD vector through the template DA vector specified by DAFSET.
//! A coefficient is kept only if the corresponding monomial is nonzero in the template.
//!
//! ## Syntax
//!
//! ```text
//! DAFILT input result;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dafilt.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{expressions::Expr, statements::SourceLocation},
    resolve::{ScopeContext, TypeResolver},
    transpile::{
        InferenceEdgeResult, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableStatement, TypeHydrationResult, TypeslotDeclarationResult, add_context_to_all,
    },
};

/// AST node for the `DAFILT input result;` filter statement.
#[derive(Debug)]
pub struct DafiltStatement {
    pub input: Expr,
    pub result: Expr,
}

impl FromRule for DafiltStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dafilt,
            "Expected `dafilt` rule when building DAFILT statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let input_pair = inner
            .next()
            .context("Missing input parameter in DAFILT statement!")?;
        let input = Expr::from_rule(input_pair)
            .context("Failed to build input expression in DAFILT statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected input expression in DAFILT statement"))?;

        let result_pair = inner
            .next()
            .context("Missing result parameter in DAFILT statement!")?;
        let result = Expr::from_rule(result_pair)
            .context("Failed to build result expression in DAFILT statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DAFILT statement"))?;

        Ok(Some(DafiltStatement { input, result }))
    }
}

impl TranspileableStatement for DafiltStatement {
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

impl Transpile for DafiltStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let input_output = self.input.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling input in DAFILT".to_string())
        })?;
        requested_variables.extend(input_output.requested_variables.iter().cloned());

        let result_output = self.result.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DAFILT".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let result_ref = result_output.as_mut_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_dafilt({}, {result_ref})?;",
            input_output.as_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
