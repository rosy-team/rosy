//! # ARGGET Statement
//!
//! Returns the n-th command line argument as a string.
//!
//! ## Syntax
//!
//! ```text
//! ARGGET n result_var;
//! ```
//!
//! - `n`          — RE expression (the zero-based index of the argument)
//! - `result_var` — ST variable that receives the argument string
//!
//! If the n-th argument does not exist, an empty string is returned.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/argget.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::{Expr, core::variable_identifier::VariableIdentifier},
    transpile::*,
};

#[derive(Debug)]
pub struct ArggetStatement {
    pub index_expr: Expr,
    pub output_var: VariableIdentifier,
}

impl FromRule for ArggetStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::argget,
            "Expected `argget` rule when building ARGGET statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let index_pair = inner
            .next()
            .context("Missing index expression in ARGGET!")?;
        let index_expr = Expr::from_rule(index_pair)
            .context("Failed to build index expression in ARGGET")?
            .ok_or_else(|| anyhow::anyhow!("Expected index expression in ARGGET"))?;

        let output_pair = inner.next().context("Missing output variable in ARGGET!")?;
        let output_var = VariableIdentifier::from_rule(output_pair)
            .context("Failed to build output variable identifier in ARGGET")?
            .ok_or_else(|| anyhow::anyhow!("Expected output variable identifier in ARGGET"))?;

        Ok(Some(ArggetStatement {
            index_expr,
            output_var,
        }))
    }
}

impl Transpile for ArggetStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let index_output = self.index_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling index expression in ARGGET".to_string(),
            )
        })?;
        requested_variables.extend(index_output.requested_variables.iter().cloned());

        let output_id_output = self.output_var.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling output variable in ARGGET".to_string(),
            )
        })?;
        requested_variables.extend(output_id_output.requested_variables.clone());

        let dereference = match context
            .variables
            .get(&self.output_var.name)
            .ok_or_else(|| {
                vec![anyhow::anyhow!(
                    "Variable '{}' is not defined in this scope!",
                    self.output_var.name
                )]
            })?
            .scope
        {
            VariableScope::Local => "",
            VariableScope::Arg => "*",
            VariableScope::Higher => {
                requested_variables.insert(self.output_var.name.clone());
                "*"
            }
        };

        let serialization = format!(
            "{deref}{dest} = std::env::args().nth(rosy_as_usize(&({src}))).unwrap_or_default();",
            deref = dereference,
            dest = output_id_output.serialization,
            src = index_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for ArggetStatement {}
