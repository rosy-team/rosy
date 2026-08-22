//! # STCRE Statement
//!
//! Converts a string to a real number.
//!
//! ## Syntax
//!
//! ```text
//! STCRE string_expr result_var;
//! ```
//!
//! - `string_expr` — ST expression (the string to parse)
//! - `result_var`  — variable that receives the RE result
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/stcre.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::{Expr, core::variable_identifier::VariableIdentifier},
    transpile::*,
};
use rosy_lib::RosyType;

#[derive(Debug)]
pub struct StcreStatement {
    pub string_expr: Expr,
    pub output_var: VariableIdentifier,
}

impl FromRule for StcreStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::stcre,
            "Expected `stcre` rule when building STCRE statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let string_pair = inner
            .next()
            .context("Missing string expression in STCRE!")?;
        let string_expr = Expr::from_rule(string_pair)
            .context("Failed to build string expression in STCRE")?
            .ok_or_else(|| anyhow::anyhow!("Expected string expression in STCRE"))?;

        let output_pair = inner.next().context("Missing output variable in STCRE!")?;
        let output_var = VariableIdentifier::from_rule(output_pair)
            .context("Failed to build output variable identifier in STCRE")?
            .ok_or_else(|| anyhow::anyhow!("Expected output variable identifier in STCRE"))?;

        Ok(Some(StcreStatement {
            string_expr,
            output_var,
        }))
    }
}

impl Transpile for StcreStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let string_output = self.string_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling string expression in STCRE".to_string(),
            )
        })?;
        requested_variables.extend(string_output.requested_variables.iter().cloned());

        let output_id_output = self.output_var.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling output variable in STCRE".to_string(),
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

        let dest_ty = context
            .variables
            .get(&self.output_var.name)
            .map(|v| v.data.r#type)
            .unwrap_or_else(RosyType::RE);
        let rhs = format!("rosy_as_f64(&({}))", string_output.as_value());
        let rhs = if dest_ty.is_any() {
            format!("RosyValue::from({rhs})")
        } else {
            rhs
        };
        let serialization = format!(
            "{deref}{dest} = {rhs};",
            deref = dereference,
            dest = output_id_output.serialization,
            rhs = rhs,
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for StcreStatement {}
