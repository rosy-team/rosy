//! # DANORO Statement (DA Remove Odd-Order Terms)
//!
//! Removes all odd-order terms from a DA vector array, keeping only even-order terms
//! (constant, quadratic, quartic, …).
//!
//! ## Syntax
//!
//! ```text
//! DANORO da_var;
//! ```
//!
//! Arguments:
//! 1. `da_var` (DA array, in/out) — DA vector to filter in place
//!
//! > **COSY note**: In COSY INFINITY, `DANORO` has completely different semantics —
//! > it computes the **maximum norms of power-sorted parts** of a DA and takes
//! > 5 arguments `(DA, var_index, array_size, norms_out, max_power_out)`.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/danoro.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement, add_context_to_all},
};

/// AST node for `DANORO da_var;`.
#[derive(Debug)]
pub struct DanoroStatement {
    pub da_expr: Expr,
}

impl FromRule for DanoroStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::danoro,
            "Expected `danoro` rule when building DANORO statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_pair = inner
            .next()
            .context("Missing da_var parameter in DANORO!")?;
        let da_expr = Expr::from_rule(da_pair)
            .context("Failed to build da_var expression in DANORO")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DANORO"))?;

        Ok(Some(DanoroStatement { da_expr }))
    }
}


impl Transpile for DanoroStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let da_output = self.da_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DANORO".to_string())
        })?;

        let da_mut = da_output.as_mut_ref();

        let serialization = format!("rosy_lib::core::da_ops::rosy_danoro({})?;", da_mut,);

        Ok(TranspilationOutput {
            serialization,
            requested_variables: da_output.requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DanoroStatement {}
