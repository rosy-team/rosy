//! # PWTIME Statement
//!
//! Returns the elapsed wall-clock time in seconds since program start.
//!
//! ## Syntax
//!
//! ```text
//! PWTIME v;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/io/pwtime.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::core::variable_identifier::VariableIdentifier,
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement, VariableScope, add_context_to_all},
};

#[derive(Debug)]
pub struct PwtimeStatement {
    pub identifier: VariableIdentifier,
}

impl FromRule for PwtimeStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::pwtime,
            "Expected `pwtime` rule when building PWTIME statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let expr_pair = inner
            .next()
            .context("Missing variable expression in PWTIME!")?;
        let identifier = VariableIdentifier::from_rule(expr_pair)
            .context("Failed to build variable identifier in PWTIME")?
            .ok_or_else(|| anyhow::anyhow!("Expected variable identifier in PWTIME"))?;

        Ok(Some(PwtimeStatement { identifier }))
    }
}


impl Transpile for PwtimeStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let output = self.identifier.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling identifier in PWTIME".to_string())
        })?;
        requested_variables.extend(output.requested_variables.clone());

        let dereference = match context
            .variables
            .get(&self.identifier.name)
            .ok_or_else(|| {
                vec![anyhow::anyhow!(
                    "Variable '{}' is not defined in this scope!",
                    self.identifier.name
                )]
            })?
            .scope
        {
            VariableScope::Local => "",
            VariableScope::Arg => "*",
            VariableScope::Higher => {
                requested_variables.insert(self.identifier.name.clone());
                "*"
            }
        };

        // Use the `start` Instant created at the top of main_wrapper() in the
        // output template — same mechanism as CPUSEC.
        let serialization = format!(
            "{}{} = start.elapsed().as_secs_f64();",
            dereference, output.serialization
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for PwtimeStatement {}
