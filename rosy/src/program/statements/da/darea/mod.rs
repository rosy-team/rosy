//! # DAREA Statement
//!
//! Reads a single DA vector from a file unit.
//! Counterpart to single-component [`super::daprv`] output.
//!
//! ## Syntax
//!
//! ```text
//! DAREA unit da_var num_vars;
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

/// AST node for `DAREA unit da_var num_vars;`.
#[derive(Debug)]
pub struct DareaStatement {
    pub unit_expr: Expr,
    pub da_var_expr: Expr,
    pub num_vars_expr: Expr,
}

impl FromRule for DareaStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::darea,
            "Expected `darea` rule when building DAREA statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let unit_pair = inner.next().context("Missing unit parameter in DAREA!")?;
        let unit_expr = Expr::from_rule(unit_pair)
            .context("Failed to build unit expression in DAREA")?
            .ok_or_else(|| anyhow::anyhow!("Expected unit expression in DAREA"))?;

        let da_var_pair = inner.next().context("Missing da_var parameter in DAREA!")?;
        let da_var_expr = Expr::from_rule(da_var_pair)
            .context("Failed to build da_var expression in DAREA")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DAREA"))?;

        let num_vars_pair = inner
            .next()
            .context("Missing num_vars parameter in DAREA!")?;
        let num_vars_expr = Expr::from_rule(num_vars_pair)
            .context("Failed to build num_vars expression in DAREA")?
            .ok_or_else(|| anyhow::anyhow!("Expected num_vars expression in DAREA"))?;

        Ok(Some(DareaStatement {
            unit_expr,
            da_var_expr,
            num_vars_expr,
        }))
    }
}

impl TranspileableStatement for DareaStatement {
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

impl Transpile for DareaStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let unit_output = self
            .unit_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling unit in DAREA".to_string()))?;
        requested_variables.extend(unit_output.requested_variables.iter().cloned());

        let da_var_output = self.da_var_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DAREA".to_string())
        })?;
        requested_variables.extend(da_var_output.requested_variables.iter().cloned());

        let num_vars_output = self.num_vars_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling num_vars in DAREA".to_string())
        })?;
        requested_variables.extend(num_vars_output.requested_variables.iter().cloned());

        let serialization = format!(
            "rosy_lib::core::dapew::rosy_darea({} as u64, {}, {} as usize)?;",
            unit_output.as_value(),
            da_var_output.as_mut_ref(),
            num_vars_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
