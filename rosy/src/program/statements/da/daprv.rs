//! # DAPRV Statement
//!
//! Prints DA (Taylor series) array components to a file unit.
//!
//! ## Syntax
//!
//! ```text
//! DAPRV array num_components max_vars current_vars unit;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/daprv.rosy"))]
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

/// AST node for `DAPRV array num_components max_vars current_vars unit;`.
/// DAPRV array num_components max_vars current_vars unit ;
#[derive(Debug)]
pub struct DaprvStatement {
    pub array_expr: Expr,
    pub num_components_expr: Expr,
    pub max_vars_expr: Expr,
    pub current_vars_expr: Expr,
    pub unit_expr: Expr,
}

impl FromRule for DaprvStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::daprv,
            "Expected `daprv` rule when building DAPRV statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let array_pair = inner.next().context("Missing array parameter in DAPRV!")?;
        let array_expr = Expr::from_rule(array_pair)
            .context("Failed to build array expression in DAPRV")?
            .ok_or_else(|| anyhow::anyhow!("Expected array expression in DAPRV"))?;

        let num_comp_pair = inner
            .next()
            .context("Missing num_components parameter in DAPRV!")?;
        let num_components_expr = Expr::from_rule(num_comp_pair)
            .context("Failed to build num_components expression in DAPRV")?
            .ok_or_else(|| anyhow::anyhow!("Expected num_components expression in DAPRV"))?;

        let max_vars_pair = inner
            .next()
            .context("Missing max_vars parameter in DAPRV!")?;
        let max_vars_expr = Expr::from_rule(max_vars_pair)
            .context("Failed to build max_vars expression in DAPRV")?
            .ok_or_else(|| anyhow::anyhow!("Expected max_vars expression in DAPRV"))?;

        let current_vars_pair = inner
            .next()
            .context("Missing current_vars parameter in DAPRV!")?;
        let current_vars_expr = Expr::from_rule(current_vars_pair)
            .context("Failed to build current_vars expression in DAPRV")?
            .ok_or_else(|| anyhow::anyhow!("Expected current_vars expression in DAPRV"))?;

        let unit_pair = inner.next().context("Missing unit parameter in DAPRV!")?;
        let unit_expr = Expr::from_rule(unit_pair)
            .context("Failed to build unit expression in DAPRV")?
            .ok_or_else(|| anyhow::anyhow!("Expected unit expression in DAPRV"))?;

        Ok(Some(DaprvStatement {
            array_expr,
            num_components_expr,
            max_vars_expr,
            current_vars_expr,
            unit_expr,
        }))
    }
}
impl Transpile for DaprvStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let array_output = self.array_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling array in DAPRV".to_string())
        })?;
        requested_variables.extend(array_output.requested_variables.iter().cloned());

        let num_comp_output = self.num_components_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling num_components in DAPRV".to_string(),
            )
        })?;
        requested_variables.extend(num_comp_output.requested_variables.iter().cloned());

        let max_vars_output = self.max_vars_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling max_vars in DAPRV".to_string())
        })?;
        requested_variables.extend(max_vars_output.requested_variables.iter().cloned());

        let current_vars_output = self.current_vars_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling current_vars in DAPRV".to_string())
        })?;
        requested_variables.extend(current_vars_output.requested_variables.iter().cloned());

        let unit_output = self
            .unit_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling unit in DAPRV".to_string()))?;
        requested_variables.extend(unit_output.requested_variables.iter().cloned());

        let serialization = format!(
            "rosy_lib::core::daprv::rosy_daprv({}, {} as usize, {} as usize, {} as usize, {} as u64)?;",
            array_output.as_ref(),
            num_comp_output.as_value(),
            max_vars_output.as_value(),
            current_vars_output.as_value(),
            unit_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DaprvStatement {}
