//! # DANOW Statement (DA Order-Weighted Norm)
//!
//! Computes the order-weighted max norm of a DA variable.
//! For each monomial of order k with coefficient c, computes |c| * k^weight,
//! then returns the maximum over all monomials.
//!
//! ## Syntax
//!
//! ```text
//! DANOW da_var weight result;
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

/// AST node for the `DANOW da_var weight result;` order-weighted norm statement.
#[derive(Debug)]
pub struct DanowStatement {
    pub da_var: Expr,
    pub weight: Expr,
    pub result: Expr,
}

impl FromRule for DanowStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::danow,
            "Expected `danow` rule when building DANOW statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_var = Expr::from_rule(inner.next().context("Missing da_var in DANOW")?)
            .context("Failed to build da_var expression in DANOW")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DANOW"))?;

        let weight = Expr::from_rule(inner.next().context("Missing weight in DANOW")?)
            .context("Failed to build weight expression in DANOW")?
            .ok_or_else(|| anyhow::anyhow!("Expected weight expression in DANOW"))?;

        let result = Expr::from_rule(inner.next().context("Missing result in DANOW")?)
            .context("Failed to build result expression in DANOW")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DANOW"))?;

        Ok(Some(DanowStatement {
            da_var,
            weight,
            result,
        }))
    }
}

impl TranspileableStatement for DanowStatement {
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

impl Transpile for DanowStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_var_output = self.da_var.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DANOW".to_string())
        })?;
        requested_variables.extend(da_var_output.requested_variables.iter().cloned());

        let weight_output = self.weight.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling weight in DANOW".to_string())
        })?;
        requested_variables.extend(weight_output.requested_variables.iter().cloned());

        let result_output = self.result.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DANOW".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let result_ref = result_output.as_mut_ref();

        let da_ref = da_var_output.as_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_danow({}, {}, {})?;",
            da_ref,
            weight_output.as_value(),
            result_ref,
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
