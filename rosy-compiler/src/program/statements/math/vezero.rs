//! # VEZERO Statement
//!
//! Sets any components of vectors in an array to zero if the component
//! exceeds a threshold value. Used in repetitive tracking to prevent
//! overflow due to lost particles.
//!
//! ## Syntax
//!
//! ```text
//! VEZERO array num_elements threshold;
//! ```
//!
//! - `array`        — variable identifier for the array of VE vectors to check (modified in-place)
//! - `num_elements` — RE expression for number of VE array elements to check
//! - `threshold`    — RE expression for the threshold value
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/vezero.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::{Expr, core::variable_identifier::VariableIdentifier},
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
        add_context_to_all,
    },
};

#[derive(Debug)]
pub struct VezeroStatement {
    pub array_ident: VariableIdentifier,
    pub num_elements_expr: Expr,
    pub threshold_expr: Expr,
}

impl FromRule for VezeroStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::vezero,
            "Expected `vezero` rule when building VEZERO statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let array_pair = inner.next().context("Missing array variable in VEZERO!")?;
        let array_ident = VariableIdentifier::from_rule(array_pair)
            .context("Failed to build array variable identifier in VEZERO")?
            .ok_or_else(|| anyhow::anyhow!("Expected array variable identifier in VEZERO"))?;

        let num_pair = inner
            .next()
            .context("Missing num_elements expression in VEZERO!")?;
        let num_elements_expr = Expr::from_rule(num_pair)
            .context("Failed to build num_elements expression in VEZERO")?
            .ok_or_else(|| anyhow::anyhow!("Expected num_elements expression in VEZERO"))?;

        let threshold_pair = inner
            .next()
            .context("Missing threshold expression in VEZERO!")?;
        let threshold_expr = Expr::from_rule(threshold_pair)
            .context("Failed to build threshold expression in VEZERO")?
            .ok_or_else(|| anyhow::anyhow!("Expected threshold expression in VEZERO"))?;

        Ok(Some(VezeroStatement {
            array_ident,
            num_elements_expr,
            threshold_expr,
        }))
    }
}

impl Transpile for VezeroStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let array_output = self.array_ident.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling array variable in VEZERO".to_string(),
            )
        })?;
        requested_variables.extend(array_output.requested_variables.clone());

        let num_output = self.num_elements_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling num_elements in VEZERO".to_string())
        })?;
        requested_variables.extend(num_output.requested_variables.iter().cloned());

        let threshold_output = self.threshold_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling threshold in VEZERO".to_string())
        })?;
        requested_variables.extend(threshold_output.requested_variables.iter().cloned());

        let serialization = format!(
            "rosy_vezero({arr}, {num}, {thresh});",
            num = num_output.as_value(),
            thresh = threshold_output.as_value(),
            arr = array_output.as_mut_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for VezeroStatement {}
