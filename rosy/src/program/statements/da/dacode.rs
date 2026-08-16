//! # DACODE Statement (DA Monomial Decode)
//!
//! Decodes the DA internal monomial numbers to exponent arrays.
//! The first argument is a parameter vector `[order, num_vars]` (validated against
//! the current DAINI setup). The second argument is the array length. The third
//! argument is a 2D RE array where the M-th row receives the exponents of the
//! M-th monomial (in graded lexicographic order).
//!
//! ## Syntax
//!
//! ```text
//! DACODE params size result;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dacode.rosy"))]
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

/// AST node for the `DACODE params size result;` monomial decode statement.
#[derive(Debug)]
pub struct DacodeStatement {
    pub params: Expr,
    pub size: Expr,
    pub result: Expr,
}

impl FromRule for DacodeStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dacode,
            "Expected `dacode` rule when building DACODE statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let params = Expr::from_rule(inner.next().context("Missing params in DACODE")?)
            .context("Failed to build params expression in DACODE")?
            .ok_or_else(|| anyhow::anyhow!("Expected params expression in DACODE"))?;

        let size = Expr::from_rule(inner.next().context("Missing size in DACODE")?)
            .context("Failed to build size expression in DACODE")?
            .ok_or_else(|| anyhow::anyhow!("Expected size expression in DACODE"))?;

        let result = Expr::from_rule(inner.next().context("Missing result in DACODE")?)
            .context("Failed to build result expression in DACODE")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DACODE"))?;

        Ok(Some(DacodeStatement {
            params,
            size,
            result,
        }))
    }
}

impl Transpile for DacodeStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let params_output = self.params.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling params in DACODE".to_string())
        })?;
        requested_variables.extend(params_output.requested_variables.iter().cloned());

        let size_output = self.size.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling size in DACODE".to_string())
        })?;
        requested_variables.extend(size_output.requested_variables.iter().cloned());

        let result_output = self.result.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DACODE".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let result_ref = result_output.as_mut_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_dacode({}, {} as usize, {result_ref})?;",
            params_output.as_ref(),
            size_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DacodeStatement {}
