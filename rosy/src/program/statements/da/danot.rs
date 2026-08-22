//! # DANOT Statement (DA Truncation Order)
//!
//! Sets the momentary truncation order for DA and CD computations.
//!
//! ## Syntax
//!
//! ```text
//! DANOT c;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/danot.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
    },
};

/// AST node for the `DANOT c;` DA truncation order statement.
#[derive(Debug)]
pub struct DanotStatement {
    pub order: Expr,
}

impl FromRule for DanotStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::danot,
            "Expected `danot` rule when building DANOT statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let order_pair = inner
            .next()
            .context("Missing truncation order parameter in DANOT statement!")?;
        let order_expr = Expr::from_rule(order_pair)
            .context("Failed to build order expression in DANOT statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected expression for order in DANOT statement"))?;

        Ok(Some(DanotStatement { order: order_expr }))
    }
}
impl Transpile for DanotStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let order_output = self.order.transpile(context).map_err(|errs| {
            errs.into_iter()
                .map(|e| e.context("...while transpiling order expression in DANOT"))
                .collect::<Vec<_>>()
        })?;

        let serialization = format!(
            "taylor::set_truncation_order(rosy_as_u32(&({})))?;",
            order_output.as_value()
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables: order_output.requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DanotStatement {}
