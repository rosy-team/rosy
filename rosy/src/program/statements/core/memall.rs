//! # MEMALL Statement
//!
//! Returns the total amount of COSY memory that is currently allocated.
//! In Rosy (Rust), there is no COSY memory pool — Rust's allocator manages memory
//! automatically, so this always returns `0.0` to indicate nothing is allocated in
//! COSY's pool.
//!
//! ## Syntax
//!
//! ```text
//! MEMALL v;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/memall.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*, program::expressions::core::variable_identifier::VariableIdentifier, transpile::*,
};

#[derive(Debug)]
pub struct MemallStatement {
    pub identifier: VariableIdentifier,
}

impl FromRule for MemallStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::memall,
            "Expected `memall` rule when building MEMALL statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let expr_pair = inner
            .next()
            .context("Missing variable expression in MEMALL!")?;
        let identifier = VariableIdentifier::from_rule(expr_pair)
            .context("Failed to build variable identifier in MEMALL")?
            .ok_or_else(|| anyhow::anyhow!("Expected variable identifier in MEMALL"))?;

        Ok(Some(MemallStatement { identifier }))
    }
}


impl Transpile for MemallStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let output = self.identifier.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling identifier in MEMALL".to_string())
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

        // Return the process's current physical memory usage via rosy_lib helper.
        // Falls back to 0.0 if the platform does not support the query.
        let serialization = format!("{}{} = rosy_memall();", dereference, output.serialization);

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for MemallStatement {}
