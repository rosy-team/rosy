//! # OPENF Statement
//!
//! Opens a text file and associates it with a unit number.
//!
//! ## Syntax
//!
//! ```text
//! OPENF unit filename status;
//! ```
//!
//! - `unit` — numeric expression for the file unit
//! - `filename` — string expression for the file path
//! - `status` — string expression (`'NEW'`, `'OLD'`, `'UNKNOWN'`, `'APPEND'`)
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/io/openf.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement, add_context_to_all},
};

/// AST node for `OPENF unit filename status;`.
/// OPENF unit filename status ;
#[derive(Debug)]
pub struct OpenfStatement {
    pub unit_expr: Expr,
    pub filename_expr: Expr,
    pub status_expr: Expr,
}

impl FromRule for OpenfStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::openf,
            "Expected `openf` rule when building OPENF statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let unit_pair = inner.next().context("Missing unit expression in OPENF!")?;
        let unit_expr = Expr::from_rule(unit_pair)
            .context("Failed to build unit expression in OPENF")?
            .ok_or_else(|| anyhow::anyhow!("Expected unit expression in OPENF"))?;

        let filename_pair = inner
            .next()
            .context("Missing filename expression in OPENF!")?;
        let filename_expr = Expr::from_rule(filename_pair)
            .context("Failed to build filename expression in OPENF")?
            .ok_or_else(|| anyhow::anyhow!("Expected filename expression in OPENF"))?;

        let status_pair = inner
            .next()
            .context("Missing status expression in OPENF!")?;
        let status_expr = Expr::from_rule(status_pair)
            .context("Failed to build status expression in OPENF")?
            .ok_or_else(|| anyhow::anyhow!("Expected status expression in OPENF"))?;

        Ok(Some(OpenfStatement {
            unit_expr,
            filename_expr,
            status_expr,
        }))
    }
}
impl Transpile for OpenfStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let unit_output = self.unit_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling unit expression in OPENF".to_string(),
            )
        })?;
        requested_variables.extend(unit_output.requested_variables.iter().cloned());

        let filename_output = self.filename_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling filename expression in OPENF".to_string(),
            )
        })?;
        requested_variables.extend(filename_output.requested_variables.iter().cloned());

        let status_output = self.status_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling status expression in OPENF".to_string(),
            )
        })?;
        requested_variables.extend(status_output.requested_variables.iter().cloned());

        let serialization = format!(
            "rosy_lib::core::file_io::rosy_openf({}, {}, {})?;",
            unit_output.as_value(),
            filename_output.as_ref(),
            status_output.as_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for OpenfStatement {}
