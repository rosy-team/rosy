//! # DAPLU Statement (DA Plug — Replace Variable with Constant)
//!
//! Replaces the power of independent variable xᵢ by the constant C.
//!
//! ## Syntax
//!
//! ```text
//! DAPLU da_in i C result;
//! ```
//!
//! Arguments:
//! 1. `da_in`  (DA vector, read)  - source DA array
//! 2. `i`      (RE, read)         - 1-based variable index
//! 3. `C`      (RE, read)         - constant to substitute for xᵢ
//! 4. `result` (DA vector, write) - result DA array
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/daplu.rosy"))]
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

/// AST node for `DAPLU da_in i C result;`.
#[derive(Debug)]
pub struct DapluStatement {
    pub da_in_expr: Expr,
    pub var_idx_expr: Expr,
    pub c_expr: Expr,
    pub result_expr: Expr,
}

impl FromRule for DapluStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::daplu,
            "Expected `daplu` rule when building DAPLU statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_in_pair = inner.next().context("Missing da_in parameter in DAPLU!")?;
        let da_in_expr = Expr::from_rule(da_in_pair)
            .context("Failed to build da_in expression in DAPLU")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_in expression in DAPLU"))?;

        let var_idx_pair = inner.next().context("Missing variable index in DAPLU!")?;
        let var_idx_expr = Expr::from_rule(var_idx_pair)
            .context("Failed to build var_idx expression in DAPLU")?
            .ok_or_else(|| anyhow::anyhow!("Expected var_idx expression in DAPLU"))?;

        let c_pair = inner.next().context("Missing C parameter in DAPLU!")?;
        let c_expr = Expr::from_rule(c_pair)
            .context("Failed to build C expression in DAPLU")?
            .ok_or_else(|| anyhow::anyhow!("Expected C expression in DAPLU"))?;

        let result_pair = inner.next().context("Missing result parameter in DAPLU!")?;
        let result_expr = Expr::from_rule(result_pair)
            .context("Failed to build result expression in DAPLU")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DAPLU"))?;

        Ok(Some(DapluStatement {
            da_in_expr,
            var_idx_expr,
            c_expr,
            result_expr,
        }))
    }
}

impl Transpile for DapluStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_in_output = self.da_in_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_in in DAPLU".to_string())
        })?;
        requested_variables.extend(da_in_output.requested_variables.iter().cloned());

        let var_idx_output = self.var_idx_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling var_idx in DAPLU".to_string())
        })?;
        requested_variables.extend(var_idx_output.requested_variables.iter().cloned());

        let c_output = self
            .c_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling C in DAPLU".to_string()))?;
        requested_variables.extend(c_output.requested_variables.iter().cloned());

        let result_output = self.result_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DAPLU".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.clone());

        // COSY's `DAPLU P K C P` aliases input and output. Rust's borrow
        // checker rejects two `&mut` (or `&` + `&mut`) to the same memory,
        // so clone the input when its underlying name matches result's.
        let result_ref = result_output.as_mut_ref();
        let da_in_ref = da_in_output.as_ref();
        let result_strip = result_ref.trim_start_matches("&mut ");
        let da_in_arg = if da_in_ref.trim_start_matches('&') == result_strip {
            format!("&{}.clone()", da_in_ref.trim_start_matches('&'))
        } else {
            da_in_ref
        };

        let serialization = format!(
            "rosy_lib::core::daprv::rosy_daplu({}, {} as usize, {} as f64, {})?;",
            da_in_arg,
            var_idx_output.as_value(),
            c_output.as_value(),
            result_ref,
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DapluStatement {}
