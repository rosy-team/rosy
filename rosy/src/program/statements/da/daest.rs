//! # DAEST Statement
//!
//! Estimates the size of j-th order terms of a DA vector (summation norm).
//! If i > 0, restricts to terms where variable i has exponent j.
//! If i == 0, sums over all terms of total order j.
//!
//! ## Syntax
//!
//! ```text
//! DAEST da_var i j result;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/daest.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement, add_context_to_all},
};

/// AST node for `DAEST da_var i j result;`.
#[derive(Debug)]
pub struct DaestStatement {
    pub da_var_expr: Expr,
    pub i_expr: Expr,
    pub j_expr: Expr,
    pub result_expr: Expr,
}

impl FromRule for DaestStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::daest,
            "Expected `daest` rule when building DAEST statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_var_pair = inner.next().context("Missing da_var parameter in DAEST!")?;
        let da_var_expr = Expr::from_rule(da_var_pair)
            .context("Failed to build da_var expression in DAEST")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DAEST"))?;

        let i_pair = inner.next().context("Missing i parameter in DAEST!")?;
        let i_expr = Expr::from_rule(i_pair)
            .context("Failed to build i expression in DAEST")?
            .ok_or_else(|| anyhow::anyhow!("Expected i expression in DAEST"))?;

        let j_pair = inner.next().context("Missing j parameter in DAEST!")?;
        let j_expr = Expr::from_rule(j_pair)
            .context("Failed to build j expression in DAEST")?
            .ok_or_else(|| anyhow::anyhow!("Expected j expression in DAEST"))?;

        let result_pair = inner.next().context("Missing result parameter in DAEST!")?;
        let result_expr = Expr::from_rule(result_pair)
            .context("Failed to build result expression in DAEST")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DAEST"))?;

        Ok(Some(DaestStatement {
            da_var_expr,
            i_expr,
            j_expr,
            result_expr,
        }))
    }
}


impl Transpile for DaestStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_var_output = self.da_var_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DAEST".to_string())
        })?;
        requested_variables.extend(da_var_output.requested_variables.iter().cloned());

        let i_output = self
            .i_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling i in DAEST".to_string()))?;
        requested_variables.extend(i_output.requested_variables.iter().cloned());

        let j_output = self
            .j_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling j in DAEST".to_string()))?;
        requested_variables.extend(j_output.requested_variables.iter().cloned());

        let result_output = self.result_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DAEST".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let serialization = format!(
            "rosy_lib::core::dapew::rosy_daest({}, {} as usize, {} as u32, {})?;",
            da_var_output.as_ref(),
            i_output.as_value(),
            j_output.as_value(),
            result_output.as_mut_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DaestStatement {}
