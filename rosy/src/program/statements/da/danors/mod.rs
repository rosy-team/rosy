//! # DANORS Statement (DA Remove Small Coefficients)
//!
//! Removes all coefficients whose absolute value is below a given threshold.
//!
//! ## Syntax
//!
//! ```text
//! DANORS da_var threshold;
//! ```
//!
//! Arguments:
//! 1. `da_var`    (DA array, in/out) — DA vector to filter in place
//! 2. `threshold` (RE)               — minimum absolute coefficient to retain
//!
//! > **COSY note**: In COSY INFINITY, `DANORS` has completely different semantics —
//! > it computes the **summation norms of power-sorted parts** of a DA and takes
//! > 5 arguments `(DA, var_index, array_size, norms_out, max_power_out)`.
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

/// AST node for `DANORS da_var threshold;`.
#[derive(Debug)]
pub struct DanorsStatement {
    pub da_expr: Expr,
    pub threshold_expr: Expr,
}

impl FromRule for DanorsStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::danors,
            "Expected `danors` rule when building DANORS statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_pair = inner
            .next()
            .context("Missing da_var parameter in DANORS!")?;
        let da_expr = Expr::from_rule(da_pair)
            .context("Failed to build da_var expression in DANORS")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DANORS"))?;

        let threshold_pair = inner
            .next()
            .context("Missing threshold parameter in DANORS!")?;
        let threshold_expr = Expr::from_rule(threshold_pair)
            .context("Failed to build threshold expression in DANORS")?
            .ok_or_else(|| anyhow::anyhow!("Expected threshold expression in DANORS"))?;

        Ok(Some(DanorsStatement {
            da_expr,
            threshold_expr,
        }))
    }
}

impl TranspileableStatement for DanorsStatement {
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

impl Transpile for DanorsStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_output = self.da_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DANORS".to_string())
        })?;
        requested_variables.extend(da_output.requested_variables.iter().cloned());

        let threshold_output = self.threshold_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling threshold in DANORS".to_string())
        })?;
        requested_variables.extend(threshold_output.requested_variables.iter().cloned());

        let da_mut = da_output.as_mut_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_danors({}, {} as f64)?;",
            da_mut,
            threshold_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
