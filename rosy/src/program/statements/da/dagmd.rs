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
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dagmd.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
        add_context_to_all,
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
            "rosy_lib::core::da_ops::rosy_dagmd({}, {}, {result_ref}, rosy_as_usize(&({})))?;",
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

impl TranspileableStatement for DagmdStatement {}
