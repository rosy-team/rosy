//! # DAPEP Statement
//!
//! Returns a parameter-dependent component of a DA vector.
//! Arguments are the DA vector, the coefficient id in TRANSPORT notation
//! for the first m variables, m, and the resulting DA vector.
//!
//! ## Syntax
//!
//! ```text
//! DAPEP da_var id m result;
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

/// AST node for `DAPEP da_var id m result;`.
#[derive(Debug)]
pub struct DapepStatement {
    pub da_var_expr: Expr,
    pub id_expr: Expr,
    pub m_expr: Expr,
    pub result_expr: Expr,
}

impl FromRule for DapepStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dapep,
            "Expected `dapep` rule when building DAPEP statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_var_pair = inner.next().context("Missing da_var parameter in DAPEP!")?;
        let da_var_expr = Expr::from_rule(da_var_pair)
            .context("Failed to build da_var expression in DAPEP")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DAPEP"))?;

        let id_pair = inner.next().context("Missing id parameter in DAPEP!")?;
        let id_expr = Expr::from_rule(id_pair)
            .context("Failed to build id expression in DAPEP")?
            .ok_or_else(|| anyhow::anyhow!("Expected id expression in DAPEP"))?;

        let m_pair = inner.next().context("Missing m parameter in DAPEP!")?;
        let m_expr = Expr::from_rule(m_pair)
            .context("Failed to build m expression in DAPEP")?
            .ok_or_else(|| anyhow::anyhow!("Expected m expression in DAPEP"))?;

        let result_pair = inner.next().context("Missing result parameter in DAPEP!")?;
        let result_expr = Expr::from_rule(result_pair)
            .context("Failed to build result expression in DAPEP")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DAPEP"))?;

        Ok(Some(DapepStatement {
            da_var_expr,
            id_expr,
            m_expr,
            result_expr,
        }))
    }
}

impl TranspileableStatement for DapepStatement {
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

impl Transpile for DapepStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_var_output = self.da_var_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DAPEP".to_string())
        })?;
        requested_variables.extend(da_var_output.requested_variables.iter().cloned());

        let id_output = self
            .id_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling id in DAPEP".to_string()))?;
        requested_variables.extend(id_output.requested_variables.iter().cloned());

        let m_output = self
            .m_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling m in DAPEP".to_string()))?;
        requested_variables.extend(m_output.requested_variables.iter().cloned());

        let result_output = self.result_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DAPEP".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let serialization = format!(
            "rosy_lib::core::dapew::rosy_dapep({}, {} as u64, {} as usize, {})?;",
            da_var_output.as_ref(),
            id_output.as_value(),
            m_output.as_value(),
            result_output.as_mut_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
