//! # DAPEE Statement
//!
//! Returns a coefficient of a DA vector identified by a TRANSPORT notation id.
//! The id encodes variable exponents as decimal digits (leftmost digit = variable 1).
//!
//! ## Syntax
//!
//! ```text
//! DAPEE da_var id result;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dapee.rosy"))]
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
            "rosy_lib::core::dapew::rosy_dapee({}, rosy_as_u64(&({})), {})?;",
            da_var_output.as_ref(),
            id_output.as_value(),
            result_output.as_mut_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DapeeStatement {}
