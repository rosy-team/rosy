//! # DADER Statement (DA Partial Derivative)
//!
//! Differentiates a DA vector array in place with respect to a variable index.
//!
//! ## Syntax
//!
//! ```text
//! DADER da_var var_index;
//! ```
//!
//! Arguments:
//! 1. `da_var`    (DA array, in/out) — DA vector to differentiate in place
//! 2. `var_index` (RE, integer)      — 1-based index of the variable to differentiate w.r.t.
//!
//! > **COSY note**: In COSY INFINITY, `DADER` takes 3 arguments `(index, input, result)`
//! > and writes to a separate output variable. Rosy's form is in-place `(da_var, index)`.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dader.rosy"))]
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

/// AST node for `DADER da_var var_index;`.
#[derive(Debug)]
pub struct DaderStatement {
    pub da_expr: Expr,
    pub index_expr: Expr,
}

impl FromRule for DaderStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dader,
            "Expected `dader` rule when building DADER statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_pair = inner.next().context("Missing da_var parameter in DADER!")?;
        let da_expr = Expr::from_rule(da_pair)
            .context("Failed to build da_var expression in DADER")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DADER"))?;

        let index_pair = inner
            .next()
            .context("Missing var_index parameter in DADER!")?;
        let index_expr = Expr::from_rule(index_pair)
            .context("Failed to build var_index expression in DADER")?
            .ok_or_else(|| anyhow::anyhow!("Expected var_index expression in DADER"))?;

        Ok(Some(DaderStatement {
            da_expr,
            index_expr,
        }))
    }
}

impl TranspileableStatement for DaderStatement {
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

impl Transpile for DaderStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_output = self.da_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DADER".to_string())
        })?;
        requested_variables.extend(da_output.requested_variables.iter().cloned());

        let index_output = self.index_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling var_index in DADER".to_string())
        })?;
        requested_variables.extend(index_output.requested_variables.iter().cloned());

        let da_mut = da_output.as_mut_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_dader({}, {} as usize)?;",
            da_mut,
            index_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
