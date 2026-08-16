//! # VEUNIT Statement
//!
//! Normalizes a vector to unit length.
//!
//! ## Syntax
//!
//! ```text
//! VEUNIT vec_in result;
//! ```
//!
//! - `vec_in` — VE expression (the vector to normalize)
//! - `result` — variable that receives the normalized VE unit vector
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/veunit.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::{Expr, core::variable_identifier::VariableIdentifier},
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
        VariableScope, add_context_to_all,
    },
};

#[derive(Debug)]
pub struct VeunitStatement {
    pub vec_expr: Expr,
    pub output_var: VariableIdentifier,
}

impl FromRule for VeunitStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::veunit,
            "Expected `veunit` rule when building VEUNIT statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let vec_pair = inner
            .next()
            .context("Missing vector expression in VEUNIT!")?;
        let vec_expr = Expr::from_rule(vec_pair)
            .context("Failed to build vector expression in VEUNIT")?
            .ok_or_else(|| anyhow::anyhow!("Expected vector expression in VEUNIT"))?;

        let output_pair = inner.next().context("Missing output variable in VEUNIT!")?;
        let output_var = VariableIdentifier::from_rule(output_pair)
            .context("Failed to build output variable identifier in VEUNIT")?
            .ok_or_else(|| anyhow::anyhow!("Expected output variable identifier in VEUNIT"))?;

        Ok(Some(VeunitStatement {
            vec_expr,
            output_var,
        }))
    }
}

impl Transpile for VeunitStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let vec_output = self.vec_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling vector in VEUNIT".to_string())
        })?;
        requested_variables.extend(vec_output.requested_variables.iter().cloned());

        let output_id_output = self.output_var.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling output variable in VEUNIT".to_string(),
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
            "{{\n    \
                let __rosy_veunit_src = {vec};\n    \
                let __rosy_veunit_norm = __rosy_veunit_src.iter().map(|x| x * x).sum::<f64>().sqrt();\n    \
                {deref}{dest} = __rosy_veunit_src.iter().map(|x| x / __rosy_veunit_norm).collect::<Vec<f64>>();\n\
            }}",
            vec = vec_output.as_value(),
            deref = dereference,
            dest = output_id_output.serialization,
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for VeunitStatement {}
