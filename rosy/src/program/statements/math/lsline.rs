//! # LSLINE Statement
//!
//! Computes the least-squares fit line y = a*x + b for n pairs of (x, y) values.
//!
//! ## Syntax
//!
//! ```text
//! LSLINE x y n a b;
//! ```
//!
//! - `x` — VE expression: the x-values array
//! - `y` — VE expression: the y-values array
//! - `n` — RE expression: number of pairs to use
//! - `a` — variable that receives the slope (RE)
//! - `b` — variable that receives the intercept (RE)
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/lsline.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::{Expr, core::variable_identifier::VariableIdentifier},
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
        ValueKind, add_context_to_all,
    },
};

/// AST node for `LSLINE x y n a b;`.
#[derive(Debug)]
pub struct LslineStatement {
    pub x_expr: Expr,
    pub y_expr: Expr,
    pub n_expr: Expr,
    pub a_var: VariableIdentifier,
    pub b_var: VariableIdentifier,
}

impl FromRule for LslineStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::lsline,
            "Expected `lsline` rule when building LSLINE statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let x_pair = inner.next().context("Missing x parameter in LSLINE!")?;
        let x_expr = Expr::from_rule(x_pair)
            .context("Failed to build x expression in LSLINE")?
            .ok_or_else(|| anyhow::anyhow!("Expected x expression in LSLINE"))?;

        let y_pair = inner.next().context("Missing y parameter in LSLINE!")?;
        let y_expr = Expr::from_rule(y_pair)
            .context("Failed to build y expression in LSLINE")?
            .ok_or_else(|| anyhow::anyhow!("Expected y expression in LSLINE"))?;

        let n_pair = inner.next().context("Missing n parameter in LSLINE!")?;
        let n_expr = Expr::from_rule(n_pair)
            .context("Failed to build n expression in LSLINE")?
            .ok_or_else(|| anyhow::anyhow!("Expected n expression in LSLINE"))?;

        let a_pair = inner
            .next()
            .context("Missing a output variable in LSLINE!")?;
        let a_var = VariableIdentifier::from_rule(a_pair)
            .context("Failed to build a variable identifier in LSLINE")?
            .ok_or_else(|| anyhow::anyhow!("Expected a variable identifier in LSLINE"))?;

        let b_pair = inner
            .next()
            .context("Missing b output variable in LSLINE!")?;
        let b_var = VariableIdentifier::from_rule(b_pair)
            .context("Failed to build b variable identifier in LSLINE")?
            .ok_or_else(|| anyhow::anyhow!("Expected b variable identifier in LSLINE"))?;

        Ok(Some(LslineStatement {
            x_expr,
            y_expr,
            n_expr,
            a_var,
            b_var,
        }))
    }
}

impl Transpile for LslineStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let x_output = self
            .x_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling x in LSLINE".to_string()))?;
        requested_variables.extend(x_output.requested_variables.iter().cloned());

        let y_output = self
            .y_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling y in LSLINE".to_string()))?;
        requested_variables.extend(y_output.requested_variables.iter().cloned());

        let n_output = self
            .n_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling n in LSLINE".to_string()))?;
        requested_variables.extend(n_output.requested_variables.iter().cloned());

        let a_output = self.a_var.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling a output variable in LSLINE".to_string(),
            )
        })?;
        requested_variables.extend(a_output.requested_variables.clone());

        let b_output = self.b_var.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling b output variable in LSLINE".to_string(),
            )
        })?;
        requested_variables.extend(b_output.requested_variables.clone());

        fn make_lvalue(ser: &str, value_kind: ValueKind, rhs: &str) -> String {
            if value_kind == ValueKind::Owned {
                format!("{ser} = {rhs}")
            } else if let Some(bare) = ser.strip_prefix('&') {
                format!("{bare} = {rhs}")
            } else {
                format!("*{ser} = {rhs}")
            }
        }

        let a_assign = make_lvalue(
            &a_output.serialization,
            a_output.value_kind,
            "rosy_lsline_a",
        );
        let b_assign = make_lvalue(
            &b_output.serialization,
            b_output.value_kind,
            "rosy_lsline_b",
        );

        let serialization = format!(
            "{{ let (rosy_lsline_a, rosy_lsline_b) = rosy_lib::core::lsline::rosy_lsline({x}, {y}, {n} as usize)?; {a_assign}; {b_assign}; }}",
            x = x_output.as_ref(),
            y = y_output.as_ref(),
            n = n_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for LslineStatement {}
