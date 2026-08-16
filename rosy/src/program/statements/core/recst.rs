//! # RECST Statement
//!
//! Converts a real (or complex) to a string using a Fortran-style format.
//!
//! ## Syntax
//!
//! ```text
//! RECST value format result_var;
//! ```
//!
//! - `value`      — RE or CM expression to convert
//! - `format`     — ST expression (Fortran format string, e.g. `'(F10.3)'`)
//! - `result_var` — variable that receives the ST result
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/recst.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::{Expr, core::variable_identifier::VariableIdentifier},
    transpile::*,
};

#[derive(Debug)]
pub struct RecstStatement {
    pub value_expr: Expr,
    pub format_expr: Expr,
    pub output_var: VariableIdentifier,
}

impl FromRule for RecstStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::recst,
            "Expected `recst` rule when building RECST statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let value_pair = inner.next().context("Missing value expression in RECST!")?;
        let value_expr = Expr::from_rule(value_pair)
            .context("Failed to build value expression in RECST")?
            .ok_or_else(|| anyhow::anyhow!("Expected value expression in RECST"))?;

        let format_pair = inner
            .next()
            .context("Missing format expression in RECST!")?;
        let format_expr = Expr::from_rule(format_pair)
            .context("Failed to build format expression in RECST")?
            .ok_or_else(|| anyhow::anyhow!("Expected format expression in RECST"))?;

        let output_pair = inner.next().context("Missing output variable in RECST!")?;
        let output_var = VariableIdentifier::from_rule(output_pair)
            .context("Failed to build output variable identifier in RECST")?
            .ok_or_else(|| anyhow::anyhow!("Expected output variable identifier in RECST"))?;

        Ok(Some(RecstStatement {
            value_expr,
            format_expr,
            output_var,
        }))
    }
}


impl Transpile for RecstStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let value_output = self.value_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling value in RECST".to_string())
        })?;
        requested_variables.extend(value_output.requested_variables.iter().cloned());

        let format_output = self.format_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling format in RECST".to_string())
        })?;
        requested_variables.extend(format_output.requested_variables.iter().cloned());

        let output_id_output = self.output_var.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling output variable in RECST".to_string(),
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

        // Parse common Fortran format specifiers and convert to Rust format
        // Supports: F (fixed), E (scientific), G (general), I (integer), A (string)
        let serialization = format!(
            "{deref}{dest} = rosy_lib::core::recst::rosy_recst({val}, &{fmt});",
            deref = dereference,
            dest = output_id_output.serialization,
            val = value_output.as_value(),
            fmt = format_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for RecstStatement {}
