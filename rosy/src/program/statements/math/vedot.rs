//! # VEDOT Statement
//!
//! Computes the scalar (inner, dot) product of two vectors.
//!
//! ## Syntax
//!
//! ```text
//! VEDOT vec1 vec2 result;
//! ```
//!
//! - `vec1`   — VE expression (first vector)
//! - `vec2`   — VE expression (second vector)
//! - `result` — variable that receives the RE dot product
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/vedot.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::{Expr, core::variable_identifier::VariableIdentifier},
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement, VariableScope, add_context_to_all},
};

#[derive(Debug)]
pub struct VedotStatement {
    pub vec1_expr: Expr,
    pub vec2_expr: Expr,
    pub output_var: VariableIdentifier,
}

impl FromRule for VedotStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::vedot,
            "Expected `vedot` rule when building VEDOT statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let vec1_pair = inner
            .next()
            .context("Missing first vector expression in VEDOT!")?;
        let vec1_expr = Expr::from_rule(vec1_pair)
            .context("Failed to build first vector expression in VEDOT")?
            .ok_or_else(|| anyhow::anyhow!("Expected first vector expression in VEDOT"))?;

        let vec2_pair = inner
            .next()
            .context("Missing second vector expression in VEDOT!")?;
        let vec2_expr = Expr::from_rule(vec2_pair)
            .context("Failed to build second vector expression in VEDOT")?
            .ok_or_else(|| anyhow::anyhow!("Expected second vector expression in VEDOT"))?;

        let output_pair = inner.next().context("Missing output variable in VEDOT!")?;
        let output_var = VariableIdentifier::from_rule(output_pair)
            .context("Failed to build output variable identifier in VEDOT")?
            .ok_or_else(|| anyhow::anyhow!("Expected output variable identifier in VEDOT"))?;

        Ok(Some(VedotStatement {
            vec1_expr,
            vec2_expr,
            output_var,
        }))
    }
}


impl Transpile for VedotStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let vec1_output = self.vec1_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling first vector in VEDOT".to_string())
        })?;
        requested_variables.extend(vec1_output.requested_variables.iter().cloned());

        let vec2_output = self.vec2_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling second vector in VEDOT".to_string())
        })?;
        requested_variables.extend(vec2_output.requested_variables.iter().cloned());

        let output_id_output = self.output_var.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling output variable in VEDOT".to_string(),
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
            "{deref}{dest} = {v1}.iter().zip({v2}.iter()).map(|(a, b)| a * b).sum::<f64>();",
            deref = dereference,
            dest = output_id_output.serialization,
            v1 = vec1_output.as_value(),
            v2 = vec2_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for VedotStatement {}
