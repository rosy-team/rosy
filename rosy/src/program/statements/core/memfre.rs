//! # MEMFRE Statement
//!
//! Returns the total amount of COSY memory that is currently still available.
//! In Rosy (Rust), there is no COSY memory pool — Rust's allocator manages memory
//! automatically, so this always returns `f64::MAX` to indicate effectively unlimited memory.
//!
//! ## Syntax
//!
//! ```text
//! MEMFRE v;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/memfre.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*, program::expressions::core::variable_identifier::VariableIdentifier, transpile::*,
};

#[derive(Debug)]
pub struct MemfreStatement {
    pub identifier: VariableIdentifier,
}

impl FromRule for MemfreStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::memfre,
            "Expected `memfre` rule when building MEMFRE statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let expr_pair = inner
            .next()
            .context("Missing variable expression in MEMFRE!")?;
        let identifier = VariableIdentifier::from_rule(expr_pair)
            .context("Failed to build variable identifier in MEMFRE")?
            .ok_or_else(|| anyhow::anyhow!("Expected variable identifier in MEMFRE"))?;

        Ok(Some(MemfreStatement { identifier }))
    }
}


impl Transpile for MemfreStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let output = self.identifier.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling identifier in MEMFRE".to_string())
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

        // Approximate "available budget" as isize::MAX minus current physical usage.
        // Falls back to f64::MAX if the platform does not support the query.
        let serialization = format!("{}{} = rosy_memfre();", dereference, output.serialization);

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for MemfreStatement {}
