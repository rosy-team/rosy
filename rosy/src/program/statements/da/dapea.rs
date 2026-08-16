//! # DAPEA Statement
//!
//! Returns a coefficient of a DA vector identified by an explicit exponent array.
//! The third argument is the size of the exponent array.
//!
//! ## Syntax
//!
//! ```text
//! DAPEA da_var exps_array size result;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dapea.rosy"))]
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

/// AST node for `DAPEA da_var exps_array size result;`.
#[derive(Debug)]
pub struct DapeaStatement {
    pub da_var_expr: Expr,
    pub exps_array_expr: Expr,
    pub size_expr: Expr,
    pub result_expr: Expr,
}

impl FromRule for DapeaStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dapea,
            "Expected `dapea` rule when building DAPEA statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_var_pair = inner.next().context("Missing da_var parameter in DAPEA!")?;
        let da_var_expr = Expr::from_rule(da_var_pair)
            .context("Failed to build da_var expression in DAPEA")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DAPEA"))?;

        let exps_array_pair = inner
            .next()
            .context("Missing exps_array parameter in DAPEA!")?;
        let exps_array_expr = Expr::from_rule(exps_array_pair)
            .context("Failed to build exps_array expression in DAPEA")?
            .ok_or_else(|| anyhow::anyhow!("Expected exps_array expression in DAPEA"))?;

        let size_pair = inner.next().context("Missing size parameter in DAPEA!")?;
        let size_expr = Expr::from_rule(size_pair)
            .context("Failed to build size expression in DAPEA")?
            .ok_or_else(|| anyhow::anyhow!("Expected size expression in DAPEA"))?;

        let result_pair = inner.next().context("Missing result parameter in DAPEA!")?;
        let result_expr = Expr::from_rule(result_pair)
            .context("Failed to build result expression in DAPEA")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DAPEA"))?;

        Ok(Some(DapeaStatement {
            da_var_expr,
            exps_array_expr,
            size_expr,
            result_expr,
        }))
    }
}

impl TranspileableStatement for DapeaStatement {
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

impl Transpile for DapeaStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_var_output = self.da_var_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DAPEA".to_string())
        })?;
        requested_variables.extend(da_var_output.requested_variables.iter().cloned());

        let exps_array_output = self.exps_array_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling exps_array in DAPEA".to_string())
        })?;
        requested_variables.extend(exps_array_output.requested_variables.iter().cloned());

        let size_output = self
            .size_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling size in DAPEA".to_string()))?;
        requested_variables.extend(size_output.requested_variables.iter().cloned());

        let result_output = self.result_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DAPEA".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let serialization = format!(
            "rosy_lib::core::dapew::rosy_dapea({}, {}, {} as usize, {})?;",
            da_var_output.as_ref(),
            exps_array_output.as_ref(),
            size_output.as_value(),
            result_output.as_mut_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
