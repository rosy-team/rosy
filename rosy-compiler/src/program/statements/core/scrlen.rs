//! # SCRLEN Statement
//!
//! Sets or queries the DA scratch arena size (COSY-compatible).
//!
//! ## Syntax
//!
//! ```text
//! SCRLEN c;
//! ```
//!
//! ## Semantics
//!
//! One in-out operation. If `c < 0`, the current size (f64 words) is written
//! into `c`. Otherwise the size is set to NINT(`c`). Default at process start
//! is 50000. Named DA values stay on the heap; operator temps use this arena.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/scrlen.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{ast::*, program::expressions::Expr, transpile::*};

/// AST node for `SCRLEN c;`.
#[derive(Debug)]
pub struct ScrlenStatement {
    pub size_expr: Expr,
}

impl FromRule for ScrlenStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::scrlen,
            "Expected `scrlen` rule when building SCRLEN statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let size_pair = inner.next().context("Missing size expression in SCRLEN!")?;
        let size_expr = Expr::from_rule(size_pair)
            .context("Failed to build size expression in SCRLEN")?
            .ok_or_else(|| anyhow::anyhow!("Expected size expression in SCRLEN"))?;

        Ok(Some(ScrlenStatement { size_expr }))
    }
}
impl Transpile for ScrlenStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let size_output = self.size_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling size expression in SCRLEN".to_string(),
            )
        })?;
        requested_variables.extend(size_output.requested_variables.iter().cloned());

        let val = size_output.as_value();
        let serialization = if let Some(name) = self.size_expr.as_bare_variable_name() {
            let var = context.variables.get(name).ok_or_else(|| {
                vec![anyhow::anyhow!(
                    "Variable '{}' is not defined in this scope!",
                    name
                )]
            })?;
            let dest = match var.scope {
                VariableScope::Local => name.to_string(),
                VariableScope::Arg => {
                    format!("(*{name})")
                }
                VariableScope::Higher => {
                    requested_variables.insert(name.to_string());
                    format!("(*{name})")
                }
            };
            format!(
                "{{ let mut __scrlen = rosy_as_f64(&({val})); rosy_scrlen(&mut __scrlen)?; {dest}.set_f64(__scrlen); }}"
            )
        } else {
            format!(
                "{{ let mut __scrlen = rosy_as_f64(&({val})); rosy_scrlen(&mut __scrlen)?; }}"
            )
        };

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for ScrlenStatement {}
