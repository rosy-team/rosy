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
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/danow.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement, add_context_to_all},
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

impl TranspileableStatement for DanowStatement {}
