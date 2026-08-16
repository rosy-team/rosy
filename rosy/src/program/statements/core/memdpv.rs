//! # MEMDPV Statement
//!
//! Dumps the memory contents of a variable for debugging.
//!
//! ## Syntax
//!
//! ```text
//! MEMDPV unit var;
//! ```
//!
//! ## Semantics in Rosy
//!
//! There is no COSY memory model in Rosy. MEMDPV is implemented as a debug
//! print that shows the variable's value using Rust's Debug formatting, writing
//! to stderr regardless of the unit number.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/memdpv.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*, program::expressions::Expr,
    transpile::*,
};

/// AST node for `MEMDPV unit var;`.
#[derive(Debug)]
pub struct MemdpvStatement {
    pub unit_expr: Expr,
    pub var_expr: Expr,
}

impl FromRule for MemdpvStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::memdpv,
            "Expected `memdpv` rule when building MEMDPV statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let unit_pair = inner.next().context("Missing unit expression in MEMDPV!")?;
        let unit_expr = Expr::from_rule(unit_pair)
            .context("Failed to build unit expression in MEMDPV")?
            .ok_or_else(|| anyhow::anyhow!("Expected unit expression in MEMDPV"))?;

        let var_pair = inner
            .next()
            .context("Missing variable expression in MEMDPV!")?;
        let var_expr = Expr::from_rule(var_pair)
            .context("Failed to build variable expression in MEMDPV")?
            .ok_or_else(|| anyhow::anyhow!("Expected variable expression in MEMDPV"))?;

        Ok(Some(MemdpvStatement {
            unit_expr,
            var_expr,
        }))
    }
}
impl Transpile for MemdpvStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let unit_output = self.unit_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling unit expression in MEMDPV".to_string(),
            )
        })?;
        requested_variables.extend(unit_output.requested_variables.iter().cloned());

        let var_output = self.var_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling variable expression in MEMDPV".to_string(),
            )
        })?;
        requested_variables.extend(var_output.requested_variables.iter().cloned());

        // MEMDPV dumps memory contents to a unit. In Rosy, print a debug representation to stderr.
        let serialization = format!(
            "{{ let _unit = {}; eprintln!(\"MEMDPV: {{:?}}\", {}); }}",
            unit_output.as_value(),
            var_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for MemdpvStatement {}
