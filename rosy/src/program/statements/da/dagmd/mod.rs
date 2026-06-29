//! # DAGMD Statement (DA Gradient-Vector Product / Lie Derivative)
//!
//! Computes the Lie derivative: `result = ∇g · f = Σᵢ (∂g/∂xᵢ) * f[i]`.
//! This is the action of the vector field `f` on the scalar function `g`.
//!
//! ## Syntax
//!
//! ```text
//! DAGMD g f result dim;
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
    program::{expressions::Expr, statements::SourceLocation},
    resolve::{ScopeContext, TypeResolver},
    transpile::{
        InferenceEdgeResult, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableStatement, TypeHydrationResult, TypeslotDeclarationResult, add_context_to_all,
    },
};

/// AST node for the `DAGMD g f result dim;` Lie derivative statement.
#[derive(Debug)]
pub struct DagmdStatement {
    pub g: Expr,
    pub f: Expr,
    pub result: Expr,
    pub dim: Expr,
}

impl FromRule for DagmdStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dagmd,
            "Expected `dagmd` rule when building DAGMD statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let g_pair = inner
            .next()
            .context("Missing g parameter in DAGMD statement!")?;
        let g = Expr::from_rule(g_pair)
            .context("Failed to build g expression in DAGMD statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected g expression in DAGMD statement"))?;

        let f_pair = inner
            .next()
            .context("Missing f parameter in DAGMD statement!")?;
        let f = Expr::from_rule(f_pair)
            .context("Failed to build f expression in DAGMD statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected f expression in DAGMD statement"))?;

        let result_pair = inner
            .next()
            .context("Missing result parameter in DAGMD statement!")?;
        let result = Expr::from_rule(result_pair)
            .context("Failed to build result expression in DAGMD statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DAGMD statement"))?;

        let dim_pair = inner
            .next()
            .context("Missing dim parameter in DAGMD statement!")?;
        let dim = Expr::from_rule(dim_pair)
            .context("Failed to build dim expression in DAGMD statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected dim expression in DAGMD statement"))?;

        Ok(Some(DagmdStatement { g, f, result, dim }))
    }
}

impl TranspileableStatement for DagmdStatement {
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

impl Transpile for DagmdStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let g_output = self
            .g
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling g in DAGMD".to_string()))?;
        requested_variables.extend(g_output.requested_variables.iter().cloned());

        let f_output = self
            .f
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling f in DAGMD".to_string()))?;
        requested_variables.extend(f_output.requested_variables.iter().cloned());

        let result_output = self.result.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DAGMD".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let dim_output = self
            .dim
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling dim in DAGMD".to_string()))?;
        requested_variables.extend(dim_output.requested_variables.iter().cloned());

        let result_ref = result_output.as_mut_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_dagmd({}, {}, {result_ref}, {} as usize)?;",
            g_output.as_ref(),
            f_output.as_ref(),
            dim_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
