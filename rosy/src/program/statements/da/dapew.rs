//! # DAPEW Statement
//!
//! Prints the part of a DA vector that has a certain order n in a
//! specified independent variable xi.
//!
//! ## Syntax
//!
//! ```text
//! DAPEW unit da_var var_i order_n;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dapew.rosy"))]
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

/// AST node for `DAPEW unit da_var var_i order_n;`.
#[derive(Debug)]
pub struct DapewStatement {
    pub unit_expr: Expr,
    pub da_var_expr: Expr,
    pub var_i_expr: Expr,
    pub order_n_expr: Expr,
}

impl FromRule for DapewStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dapew,
            "Expected `dapew` rule when building DAPEW statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let unit_pair = inner.next().context("Missing unit parameter in DAPEW!")?;
        let unit_expr = Expr::from_rule(unit_pair)
            .context("Failed to build unit expression in DAPEW")?
            .ok_or_else(|| anyhow::anyhow!("Expected unit expression in DAPEW"))?;

        let da_var_pair = inner.next().context("Missing da_var parameter in DAPEW!")?;
        let da_var_expr = Expr::from_rule(da_var_pair)
            .context("Failed to build da_var expression in DAPEW")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DAPEW"))?;

        let var_i_pair = inner.next().context("Missing var_i parameter in DAPEW!")?;
        let var_i_expr = Expr::from_rule(var_i_pair)
            .context("Failed to build var_i expression in DAPEW")?
            .ok_or_else(|| anyhow::anyhow!("Expected var_i expression in DAPEW"))?;

        let order_n_pair = inner
            .next()
            .context("Missing order_n parameter in DAPEW!")?;
        let order_n_expr = Expr::from_rule(order_n_pair)
            .context("Failed to build order_n expression in DAPEW")?
            .ok_or_else(|| anyhow::anyhow!("Expected order_n expression in DAPEW"))?;

        Ok(Some(DapewStatement {
            unit_expr,
            da_var_expr,
            var_i_expr,
            order_n_expr,
        }))
    }
}

impl Transpile for DapewStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let unit_output = self
            .unit_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling unit in DAPEW".to_string()))?;
        requested_variables.extend(unit_output.requested_variables.iter().cloned());

        let da_var_output = self.da_var_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DAPEW".to_string())
        })?;
        requested_variables.extend(da_var_output.requested_variables.iter().cloned());

        let var_i_output = self.var_i_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling var_i in DAPEW".to_string())
        })?;
        requested_variables.extend(var_i_output.requested_variables.iter().cloned());

        let order_n_output = self.order_n_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling order_n in DAPEW".to_string())
        })?;
        requested_variables.extend(order_n_output.requested_variables.iter().cloned());

        let serialization = format!(
            "rosy_lib::core::dapew::rosy_dapew(rosy_as_u64(&({})), {}, rosy_as_usize(&({})), rosy_as_u32(&({})))?;",
            unit_output.as_value(),
            da_var_output.as_ref(),
            var_i_output.as_value(),
            order_n_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DapewStatement {}
