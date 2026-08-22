//! # SLEEPM Statement
//!
//! Suspends program execution for a given duration in milliseconds.
//!
//! ## Syntax
//!
//! ```text
//! SLEEPM c;
//! ```
//!
//! ## Semantics in Rosy
//!
//! Transpiles to `std::thread::sleep(std::time::Duration::from_millis(...))`.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/sleepm.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{ast::*, program::expressions::Expr, transpile::*};

/// AST node for `SLEEPM c;`.
#[derive(Debug)]
pub struct SleepmStatement {
    pub duration_expr: Expr,
}

impl FromRule for SleepmStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::sleepm,
            "Expected `sleepm` rule when building SLEEPM statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let duration_pair = inner
            .next()
            .context("Missing duration expression in SLEEPM!")?;
        let duration_expr = Expr::from_rule(duration_pair)
            .context("Failed to build duration expression in SLEEPM")?
            .ok_or_else(|| anyhow::anyhow!("Expected duration expression in SLEEPM"))?;

        Ok(Some(SleepmStatement { duration_expr }))
    }
}
impl Transpile for SleepmStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let duration_output = self.duration_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling duration expression in SLEEPM".to_string(),
            )
        })?;
        requested_variables.extend(duration_output.requested_variables.iter().cloned());

        let serialization = format!(
            "std::thread::sleep(std::time::Duration::from_millis(rosy_as_u64(&({}))));",
            duration_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for SleepmStatement {}
