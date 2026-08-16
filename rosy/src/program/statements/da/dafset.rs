//! # DAFSET Statement (DA Filter Set)
//!
//! Sets the DA filtering template used by DAFILT. Provide a template DA vector;
//! pass `0` (a scalar zero DA) to disable filtering.
//!
//! ## Syntax
//!
//! ```text
//! DAFSET template_da;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dafset.rosy"))]
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

/// AST node for the `DAFSET template_da;` filter set statement.
#[derive(Debug)]
pub struct DafsetStatement {
    pub template: Expr,
}

impl FromRule for DafsetStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dafset,
            "Expected `dafset` rule when building DAFSET statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let template_pair = inner
            .next()
            .context("Missing template parameter in DAFSET statement!")?;
        let template = Expr::from_rule(template_pair)
            .context("Failed to build template expression in DAFSET statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected template expression in DAFSET statement"))?;

        Ok(Some(DafsetStatement { template }))
    }
}

impl TranspileableStatement for DafsetStatement {
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

impl Transpile for DafsetStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let template_output = self.template.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling template in DAFSET".to_string())
        })?;
        requested_variables.extend(template_output.requested_variables.iter().cloned());

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_dafset({}.clone())?;",
            template_output.as_value()
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
