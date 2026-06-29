//! # DAPEE Statement
//!
//! Returns a coefficient of a DA vector identified by a TRANSPORT notation id.
//! The id encodes a COSY/TRANSPORT variable list. Repeated digits raise that
//! variable's exponent; for example, `222` addresses the x_2^3 coefficient.
//!
//! ## Syntax
//!
//! ```text
//! DAPEE da_var id result;
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

/// AST node for `DAPEE da_var id result;`.
#[derive(Debug)]
pub struct DapeeStatement {
    pub da_var_expr: Expr,
    pub id_expr: Expr,
    pub result_expr: Expr,
}

impl FromRule for DapeeStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dapee,
            "Expected `dapee` rule when building DAPEE statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_var_pair = inner.next().context("Missing da_var parameter in DAPEE!")?;
        let da_var_expr = Expr::from_rule(da_var_pair)
            .context("Failed to build da_var expression in DAPEE")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DAPEE"))?;

        let id_pair = inner.next().context("Missing id parameter in DAPEE!")?;
        let id_expr = Expr::from_rule(id_pair)
            .context("Failed to build id expression in DAPEE")?
            .ok_or_else(|| anyhow::anyhow!("Expected id expression in DAPEE"))?;

        let result_pair = inner.next().context("Missing result parameter in DAPEE!")?;
        let result_expr = Expr::from_rule(result_pair)
            .context("Failed to build result expression in DAPEE")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DAPEE"))?;

        Ok(Some(DapeeStatement {
            da_var_expr,
            id_expr,
            result_expr,
        }))
    }
}

impl TranspileableStatement for DapeeStatement {
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

impl Transpile for DapeeStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_var_output = self.da_var_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DAPEE".to_string())
        })?;
        requested_variables.extend(da_var_output.requested_variables.iter().cloned());

        let id_output = self
            .id_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling id in DAPEE".to_string()))?;
        requested_variables.extend(id_output.requested_variables.iter().cloned());

        let result_output = self.result_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DAPEE".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let serialization = format!(
            "rosy_lib::core::dapew::rosy_dapee({}, {} as u64, {})?;",
            da_var_output.as_ref(),
            id_output.as_value(),
            result_output
                .as_mut_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
