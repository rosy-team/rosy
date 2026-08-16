//! # EPSMIN Statement (Machine Underflow Threshold)
//!
//! Returns the underflow threshold — the smallest positive number representable
//! on the system (`f64::MIN_POSITIVE`, equivalent to `2.225e-308`).
//!
//! ## Syntax
//!
//! ```text
//! EPSMIN v;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/epsmin.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement, add_context_to_all},
};

/// AST node for the `EPSMIN v;` machine underflow threshold statement.
#[derive(Debug)]
pub struct EpsminStatement {
    pub result: Expr,
}

impl FromRule for EpsminStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::epsmin,
            "Expected `epsmin` rule when building EPSMIN statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let result_pair = inner
            .next()
            .context("Missing result variable in EPSMIN statement!")?;
        let result = Expr::from_rule(result_pair)
            .context("Failed to build result expression in EPSMIN statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in EPSMIN statement"))?;

        Ok(Some(EpsminStatement { result }))
    }
}


impl Transpile for EpsminStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let result_output = self.result.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in EPSMIN".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let result_ref = result_output.as_mut_ref();

        let serialization = format!("*{result_ref} = f64::MIN_POSITIVE;");

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for EpsminStatement {}
