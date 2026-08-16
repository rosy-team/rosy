//! # INTPOL Statement
//!
//! Computes the coefficients of the unique degree-n polynomial P(x) satisfying
//! P(1) = 1 and P^(i)(1) = 0 for i = 1, ..., n.
//!
//! ## Syntax
//!
//! ```text
//! INTPOL v c;
//! ```
//!
//! | Arg | Role                                              |
//! |-----|---------------------------------------------------|
//! | v   | coefficient array (VE, output), size n+1          |
//! | c   | order n (RE, used as integer)                     |
//!
//! The unique degree-n polynomial satisfying all n+1 conditions at x=1 is P(x) = 1.
//! Thus the coefficient array is [1, 0, 0, ..., 0] (n+1 elements).
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/intpol.rosy"))]
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

/// AST node for `INTPOL v c;`.
#[derive(Debug)]
pub struct IntpolStatement {
    /// v — coefficient array (VE output variable)
    pub coeff_var: VariableIdentifier,
    /// c — order n (RE expression, used as integer)
    pub n_expr: Expr,
}

impl FromRule for IntpolStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::intpol,
            "Expected `intpol` rule when building INTPOL statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let coeff_pair = inner
            .next()
            .context("Missing coefficient array variable in INTPOL!")?;
        let coeff_var = VariableIdentifier::from_rule(coeff_pair)
            .context("Failed to build coefficient array variable identifier in INTPOL")?
            .ok_or_else(|| {
                anyhow::anyhow!("Expected coefficient array variable identifier in INTPOL")
            })?;

        let n_pair = inner
            .next()
            .context("Missing order expression in INTPOL!")?;
        let n_expr = Expr::from_rule(n_pair)
            .context("Failed to build order expression in INTPOL")?
            .ok_or_else(|| anyhow::anyhow!("Expected order expression in INTPOL"))?;

        Ok(Some(IntpolStatement { coeff_var, n_expr }))
    }
}

impl Transpile for IntpolStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let coeff_out = self.coeff_var.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling coefficient variable in INTPOL".to_string(),
            )
        })?;
        requested_variables.extend(coeff_out.requested_variables.clone());

        let n_out = self.n_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling order expression in INTPOL".to_string(),
            )
        })?;
        requested_variables.extend(n_out.requested_variables.iter().cloned());

        let dereference = match context
            .variables
            .get(&self.coeff_var.name)
            .ok_or_else(|| {
                vec![anyhow::anyhow!(
                    "Variable '{}' is not defined in this scope!",
                    self.coeff_var.name
                )]
            })?
            .scope
        {
            VariableScope::Local => "",
            VariableScope::Arg => "*",
            VariableScope::Higher => {
                requested_variables.insert(self.coeff_var.name.clone());
                "*"
            }
        };

        // P(x) = 1 uniquely satisfies P(1)=1, P'(1)=0, ..., P^(n)(1)=0 for a degree-n polynomial.
        // Grow the vec to n+1 elements if smaller (VARIABLE (VE) starts empty), then write in-place.
        // Parens around `({deref}{dest})` are essential when scope is Arg/Higher: bare
        // `*PN.len()` parses as `*(PN.len())` (deref a usize), not `(*PN).len()`.
        let serialization = format!(
            "{{ let __intpol_n = ({n}) as usize; if ({deref}{dest}).len() < __intpol_n + 1 {{ ({deref}{dest}).resize(__intpol_n + 1, 0.0_f64); }} for __i in 0..(__intpol_n + 1) {{ ({deref}{dest})[__i] = 0.0_f64; }} if !({deref}{dest}).is_empty() {{ ({deref}{dest})[0] = 1.0_f64; }} }}",
            n = n_out.as_value(),
            deref = dereference,
            dest = coeff_out.serialization,
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for IntpolStatement {}
