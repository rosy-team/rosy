//! # DACLIW Statement (DA Extract Linear Coefficients)
//!
//! Extracts the first-order (linear) coefficients of a DA vector into an array.
//! When order-weighted DA is in use, the weighted linear coefficients are extracted.
//!
//! ## Syntax
//!
//! ```text
//! DACLIW da n linear;
//! ```
//!
//! Arguments:
//! 1. `da`     (DA vector, read)  - source DA (first component used)
//! 2. `n`      (RE, read)         - number of coefficients to extract
//! 3. `linear` (VE, write)        - output array of n linear coefficients
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dacliw.rosy"))]
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

/// AST node for `DACLIW da n linear;`.
#[derive(Debug)]
pub struct DacliwStatement {
    pub da_expr: Expr,
    pub n_expr: Expr,
    pub linear_expr: Expr,
}

impl FromRule for DacliwStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dacliw,
            "Expected `dacliw` rule when building DACLIW statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_pair = inner.next().context("Missing da parameter in DACLIW!")?;
        let da_expr = Expr::from_rule(da_pair)
            .context("Failed to build da expression in DACLIW")?
            .ok_or_else(|| anyhow::anyhow!("Expected da expression in DACLIW"))?;

        let n_pair = inner.next().context("Missing n parameter in DACLIW!")?;
        let n_expr = Expr::from_rule(n_pair)
            .context("Failed to build n expression in DACLIW")?
            .ok_or_else(|| anyhow::anyhow!("Expected n expression in DACLIW"))?;

        let linear_pair = inner
            .next()
            .context("Missing linear parameter in DACLIW!")?;
        let linear_expr = Expr::from_rule(linear_pair)
            .context("Failed to build linear expression in DACLIW")?
            .ok_or_else(|| anyhow::anyhow!("Expected linear expression in DACLIW"))?;

        Ok(Some(DacliwStatement {
            da_expr,
            n_expr,
            linear_expr,
        }))
    }
}

impl Transpile for DacliwStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_output = self
            .da_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling da in DACLIW".to_string()))?;
        requested_variables.extend(da_output.requested_variables.iter().cloned());

        let n_output = self
            .n_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling n in DACLIW".to_string()))?;
        requested_variables.extend(n_output.requested_variables.iter().cloned());

        let linear_output = self.linear_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling linear in DACLIW".to_string())
        })?;
        requested_variables.extend(linear_output.requested_variables.clone());

        let serialization = format!(
            "rosy_lib::core::daprv::rosy_dacliw({}, {} as usize, {})?;",
            da_output.as_ref(),
            n_output.as_value(),
            linear_output.as_mut_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DacliwStatement {}
