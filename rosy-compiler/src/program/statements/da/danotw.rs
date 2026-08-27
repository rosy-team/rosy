//! # DANOTW Statement (DA Weighted Order)
//!
//! Sets per-variable weight factors for DA and CD monomial ordering.
//! Must be called before DAINI. The next `DAINI` call enumerates monomials
//! where `Σ wᵢ·eᵢ ≤ max_order`, then clears the weight vector.
//!
//! ## Syntax
//!
//! ```text
//! DANOTW weights size;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/danotw.rosy"))]
//! ```

use anyhow::{Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
        add_context_to_all,
    },
};

/// AST node for the `DANOTW weights size;` weighted order statement.
#[derive(Debug)]
pub struct DanotwStatement {
    pub weights: Expr,
    pub size: Expr,
}

impl FromRule for DanotwStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::danotw,
            "Expected `danotw` rule when building DANOTW statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let weights = Expr::from_rule(
            inner
                .next()
                .ok_or_else(|| anyhow::anyhow!("Missing weights in DANOTW"))?,
        )?
        .ok_or_else(|| anyhow::anyhow!("Expected weights expression in DANOTW"))?;

        let size = Expr::from_rule(
            inner
                .next()
                .ok_or_else(|| anyhow::anyhow!("Missing size in DANOTW"))?,
        )?
        .ok_or_else(|| anyhow::anyhow!("Expected size expression in DANOTW"))?;

        Ok(Some(DanotwStatement { weights, size }))
    }
}

impl Transpile for DanotwStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let weights_output = self.weights.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling weights in DANOTW".to_string())
        })?;
        requested_variables.extend(weights_output.requested_variables.iter().cloned());

        let size_output = self.size.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling size in DANOTW".to_string())
        })?;
        requested_variables.extend(size_output.requested_variables.iter().cloned());

        let serialization = format!(
            "{{\n\t\t\tlet __danotw_weights = {weights};\n\t\t\tlet __danotw_size = rosy_as_usize(&({size}));\n\t\t\tlet __danotw_vec: Vec<u32> = __danotw_weights.iter().take(__danotw_size).map(|&v| v as u32).collect();\n\t\t\ttaylor::set_weight_vector(__danotw_vec)?;\n\t\t}}",
            weights = weights_output.as_value(),
            size = size_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DanotwStatement {}
