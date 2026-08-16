//! # DARAN Statement (DA Random Fill)
//!
//! Fills a DA vector with random entries between -1 and 1.
//! The sparsity factor controls what fraction of monomials are set nonzero.
//!
//! ## Syntax
//!
//! ```text
//! DARAN da_var sparsity;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/daran.rosy"))]
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

/// AST node for the `DARAN da_var sparsity;` random DA fill statement.
#[derive(Debug)]
pub struct DaranStatement {
    pub da_var: Expr,
    pub sparsity: Expr,
}

impl FromRule for DaranStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::daran,
            "Expected `daran` rule when building DARAN statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_var_pair = inner.next().context("Missing da_var in DARAN statement!")?;
        let da_var = Expr::from_rule(da_var_pair)
            .context("Failed to build da_var expression in DARAN statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DARAN statement"))?;

        let sparsity_pair = inner
            .next()
            .context("Missing sparsity in DARAN statement!")?;
        let sparsity = Expr::from_rule(sparsity_pair)
            .context("Failed to build sparsity expression in DARAN statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected sparsity expression in DARAN statement"))?;

        Ok(Some(DaranStatement { da_var, sparsity }))
    }
}

impl TranspileableStatement for DaranStatement {
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

impl Transpile for DaranStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_var_output = self.da_var.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DARAN".to_string())
        })?;
        requested_variables.extend(da_var_output.requested_variables.iter().cloned());

        let sparsity_output = self.sparsity.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling sparsity in DARAN".to_string())
        })?;
        requested_variables.extend(sparsity_output.requested_variables.iter().cloned());

        let da_ref = da_var_output.as_mut_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_daran({da_ref}, {} as f64)?;",
            sparsity_output.as_value()
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
