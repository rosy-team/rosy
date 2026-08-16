//! # PNPRO Statement
//!
//! Returns the number of concurrent processes. Always 1 in serial mode.
//!
//! ## Syntax
//!
//! ```text
//! PNPRO v;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/pnpro.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*, program::expressions::core::variable_identifier::VariableIdentifier, transpile::*,
};

#[derive(Debug)]
pub struct PnproStatement {
    pub identifier: VariableIdentifier,
}

impl FromRule for PnproStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::pnpro,
            "Expected `pnpro` rule when building PNPRO statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let expr_pair = inner
            .next()
            .context("Missing variable expression in PNPRO!")?;
        let identifier = VariableIdentifier::from_rule(expr_pair)
            .context("Failed to build variable identifier in PNPRO")?
            .ok_or_else(|| anyhow::anyhow!("Expected variable identifier in PNPRO"))?;

        Ok(Some(PnproStatement { identifier }))
    }
}


impl Transpile for PnproStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let output = self.identifier.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling identifier in PNPRO".to_string())
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

        // Returns the total number of MPI processes. In serial mode (no
        // `mpirun`), MPI initializes with size = 1, so this still yields 1.0.
        requested_variables.insert("rosy_mpi_context".to_string());
        let serialization = format!(
            "{}{} = rosy_mpi_context.size as f64;",
            dereference, output.serialization
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for PnproStatement {}
