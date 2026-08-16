//! # DACQLC Statement (DA Extract Quadratic Lie Coefficients)
//!
//! Extracts coefficients up to second order from a DA vector.
//! Decomposes the first DA component as: xᵀHx/2 + Lx + c
//!
//! ## Syntax
//!
//! ```text
//! DACQLC da n hessian linear constant;
//! ```
//!
//! Arguments:
//! 1. `da`       (DA vector, read)    - source DA (first component used)
//! 2. `n`        (RE, read)           - size of the linear and Hessian arrays
//! 3. `hessian`  (RE n×n, write)      - n×n Hessian matrix H
//! 4. `linear`   (VE, write)          - n-element vector L
//! 5. `constant` (RE, write)          - scalar constant c
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dacqlc.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
        add_context_to_all,
    },
};

/// AST node for `DACQLC da n hessian linear constant;`.
#[derive(Debug)]
pub struct DacqlcStatement {
    pub da_expr: Expr,
    pub n_expr: Expr,
    pub hessian_expr: Expr,
    pub linear_expr: Expr,
    pub constant_expr: Expr,
}

impl FromRule for DacqlcStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dacqlc,
            "Expected `dacqlc` rule when building DACQLC statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_pair = inner.next().context("Missing da parameter in DACQLC!")?;
        let da_expr = Expr::from_rule(da_pair)
            .context("Failed to build da expression in DACQLC")?
            .ok_or_else(|| anyhow::anyhow!("Expected da expression in DACQLC"))?;

        let n_pair = inner.next().context("Missing n parameter in DACQLC!")?;
        let n_expr = Expr::from_rule(n_pair)
            .context("Failed to build n expression in DACQLC")?
            .ok_or_else(|| anyhow::anyhow!("Expected n expression in DACQLC"))?;

        let hessian_pair = inner
            .next()
            .context("Missing hessian parameter in DACQLC!")?;
        let hessian_expr = Expr::from_rule(hessian_pair)
            .context("Failed to build hessian expression in DACQLC")?
            .ok_or_else(|| anyhow::anyhow!("Expected hessian expression in DACQLC"))?;

        let linear_pair = inner
            .next()
            .context("Missing linear parameter in DACQLC!")?;
        let linear_expr = Expr::from_rule(linear_pair)
            .context("Failed to build linear expression in DACQLC")?
            .ok_or_else(|| anyhow::anyhow!("Expected linear expression in DACQLC"))?;

        let constant_pair = inner
            .next()
            .context("Missing constant parameter in DACQLC!")?;
        let constant_expr = Expr::from_rule(constant_pair)
            .context("Failed to build constant expression in DACQLC")?
            .ok_or_else(|| anyhow::anyhow!("Expected constant expression in DACQLC"))?;

        Ok(Some(DacqlcStatement {
            da_expr,
            n_expr,
            hessian_expr,
            linear_expr,
            constant_expr,
        }))
    }
}

impl Transpile for DacqlcStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_output = self
            .da_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling da in DACQLC".to_string()))?;
        requested_variables.extend(da_output.requested_variables.iter().cloned());

        let n_output = self
            .n_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling n in DACQLC".to_string()))?;
        requested_variables.extend(n_output.requested_variables.iter().cloned());

        let hessian_output = self.hessian_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling hessian in DACQLC".to_string())
        })?;
        requested_variables.extend(hessian_output.requested_variables.iter().cloned());

        let linear_output = self.linear_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling linear in DACQLC".to_string())
        })?;
        requested_variables.extend(linear_output.requested_variables.iter().cloned());

        let constant_output = self.constant_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling constant in DACQLC".to_string())
        })?;
        requested_variables.extend(constant_output.requested_variables.clone());

        let serialization = format!(
            "rosy_lib::core::daprv::rosy_dacqlc({}, {} as usize, {}, {}, {})?;",
            da_output.as_ref(),
            n_output.as_value(),
            hessian_output.as_mut_ref(),
            linear_output.as_mut_ref(),
            constant_output.as_mut_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DacqlcStatement {}
