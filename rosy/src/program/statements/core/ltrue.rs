//! # LTRUE Statement
//!
//! Returns the logical value true, assigning `true` to the given LO variable.
//!
//! ## Syntax
//!
//! ```text
//! LTRUE v;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/ltrue.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*, program::expressions::core::variable_identifier::VariableIdentifier, transpile::*,
};

#[derive(Debug)]
pub struct LtrueStatement {
    pub identifier: VariableIdentifier,
}

impl FromRule for LtrueStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::ltrue,
            "Expected `ltrue` rule when building LTRUE statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let expr_pair = inner
            .next()
            .context("Missing variable expression in LTRUE!")?;
        let identifier = VariableIdentifier::from_rule(expr_pair)
            .context("Failed to build variable identifier in LTRUE")?
            .ok_or_else(|| anyhow::anyhow!("Expected variable identifier in LTRUE"))?;

        Ok(Some(LtrueStatement { identifier }))
    }
}


impl Transpile for LtrueStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let output = self.identifier.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling identifier in LTRUE".to_string())
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

        let serialization = format!("{}{} = true;", dereference, output.serialization);

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for LtrueStatement {}
