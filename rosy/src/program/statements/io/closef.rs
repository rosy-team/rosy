//! # CLOSEF Statement
//!
//! Closes a file unit previously opened with `OPENF` or `OPENFB`.
//!
//! ## Syntax
//!
//! ```text
//! CLOSEF unit;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/io/closef.rosy"))]
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

/// AST node for `CLOSEF unit;`.
/// CLOSEF unit ;
#[derive(Debug)]
pub struct ClosefStatement {
    pub unit_expr: Expr,
}

impl FromRule for ClosefStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::closef,
            "Expected `closef` rule when building CLOSEF statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let unit_pair = inner.next().context("Missing unit expression in CLOSEF!")?;
        let unit_expr = Expr::from_rule(unit_pair)
            .context("Failed to build unit expression in CLOSEF")?
            .ok_or_else(|| anyhow::anyhow!("Expected unit expression in CLOSEF"))?;

        Ok(Some(ClosefStatement { unit_expr }))
    }
}
impl Transpile for ClosefStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let unit_output = self.unit_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling unit expression in CLOSEF".to_string(),
            )
        })?;
        requested_variables.extend(unit_output.requested_variables.iter().cloned());

        let serialization = format!(
            "rosy_lib::core::file_io::rosy_closef({})?;",
            unit_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for ClosefStatement {}
