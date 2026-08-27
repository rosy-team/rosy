//! # REWF Statement
//!
//! Rewinds a file unit previously opened with `OPENF` or `OPENFB`.
//!
//! ## Syntax
//!
//! ```text
//! REWF unit;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/io/rewf.rosy"))]
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

/// AST node for `REWF unit;`.
/// REWF unit ;
#[derive(Debug)]
pub struct RewfStatement {
    pub unit_expr: Expr,
}

impl FromRule for RewfStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::rewf,
            "Expected `rewf` rule when building REWF statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let unit_pair = inner.next().context("Missing unit expression in REWF!")?;
        let unit_expr = Expr::from_rule(unit_pair)
            .context("Failed to build unit expression in REWF")?
            .ok_or_else(|| anyhow::anyhow!("Expected unit expression in REWF"))?;

        Ok(Some(RewfStatement { unit_expr }))
    }
}
impl Transpile for RewfStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let unit_output = self.unit_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling unit expression in REWF".to_string(),
            )
        })?;
        requested_variables.extend(unit_output.requested_variables.iter().cloned());

        let serialization = format!(
            "rosy_lib::core::file_io::rosy_rewf({})?;",
            unit_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for RewfStatement {}
