//! # DADIU Statement (DA Division by Independent Variable)
//!
//! Divides a DA vector by independent variable xᵢ if possible.
//! Terms whose xᵢ-exponent is zero are dropped (contribute 0 to result).
//!
//! ## Syntax
//!
//! ```text
//! DADIU i da_in result;
//! ```
//!
//! Arguments:
//! 1. `i`      (RE, read)         - 1-based variable index
//! 2. `da_in`  (DA vector, read)  - source DA array
//! 3. `result` (DA vector, write) - result DA array
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dadiu.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    resolve::{ScopeContext, TypeResolver},
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
        add_context_to_all,
    },
};

/// AST node for `DADIU i da_in result;`.
#[derive(Debug)]
pub struct DadiuStatement {
    pub var_idx_expr: Expr,
    pub da_in_expr: Expr,
    pub result_expr: Expr,
}

impl FromRule for DadiuStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dadiu,
            "Expected `dadiu` rule when building DADIU statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let var_idx_pair = inner.next().context("Missing variable index in DADIU!")?;
        let var_idx_expr = Expr::from_rule(var_idx_pair)
            .context("Failed to build var_idx expression in DADIU")?
            .ok_or_else(|| anyhow::anyhow!("Expected var_idx expression in DADIU"))?;

        let da_in_pair = inner.next().context("Missing da_in parameter in DADIU!")?;
        let da_in_expr = Expr::from_rule(da_in_pair)
            .context("Failed to build da_in expression in DADIU")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_in expression in DADIU"))?;

        let result_pair = inner.next().context("Missing result parameter in DADIU!")?;
        let result_expr = Expr::from_rule(result_pair)
            .context("Failed to build result expression in DADIU")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DADIU"))?;

        Ok(Some(DadiuStatement {
            var_idx_expr,
            da_in_expr,
            result_expr,
        }))
    }
}

impl Transpile for DadiuStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let var_idx_output = self.var_idx_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling var_idx in DADIU".to_string())
        })?;
        requested_variables.extend(var_idx_output.requested_variables.iter().cloned());

        let da_in_output = self.da_in_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_in in DADIU".to_string())
        })?;
        requested_variables.extend(da_in_output.requested_variables.iter().cloned());

        let result_output = self.result_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DADIU".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.clone());

        let serialization = format!(
            "rosy_lib::core::daprv::rosy_dadiu(rosy_as_usize(&({})), {}, {})?;",
            var_idx_output.as_value(),
            da_in_output.as_ref(),
            result_output.as_mut_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DadiuStatement {
    fn wire_inference_edges(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        _source_location: crate::program::statements::SourceLocation,
    ) -> Option<Result<()>> {
        super::wire_da_result_cell(&self.result_expr, resolver, ctx, "DADIU");
        Some(Ok(()))
    }
}
