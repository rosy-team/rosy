//! # LEV Statement
//!
//! Computes eigenvalues and eigenvectors of a matrix.
//!
//! ## Syntax
//!
//! ```text
//! LEV matrix eig_real eig_imag eigvecs n alloc_dim;
//! ```
//!
//! - `matrix`    — input matrix (RE ** 2)
//! - `eig_real`  — variable for real parts of eigenvalues (RE ** 1)
//! - `eig_imag`  — variable for imaginary parts of eigenvalues (RE ** 1)
//! - `eigvecs`   — variable for eigenvector matrix (RE ** 2)
//! - `n`         — number of actual entries (RE, used as usize)
//! - `alloc_dim` — allocation dimension (RE, used as usize)
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/lev.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
        ValueKind, add_context_to_all,
    },
};

#[derive(Debug)]
pub struct LevStatement {
    pub matrix_expr: Expr,
    pub eig_real_expr: Expr,
    pub eig_imag_expr: Expr,
    pub eigvecs_expr: Expr,
    pub n_expr: Expr,
    pub alloc_dim_expr: Expr,
}

impl FromRule for LevStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::lev,
            "Expected `lev` rule when building LEV statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let matrix_pair = inner.next().context("Missing matrix parameter in LEV!")?;
        let matrix_expr = Expr::from_rule(matrix_pair)
            .context("Failed to build matrix expression in LEV")?
            .ok_or_else(|| anyhow::anyhow!("Expected matrix expression in LEV"))?;

        let eig_real_pair = inner.next().context("Missing eig_real parameter in LEV!")?;
        let eig_real_expr = Expr::from_rule(eig_real_pair)
            .context("Failed to build eig_real expression in LEV")?
            .ok_or_else(|| anyhow::anyhow!("Expected eig_real expression in LEV"))?;

        let eig_imag_pair = inner.next().context("Missing eig_imag parameter in LEV!")?;
        let eig_imag_expr = Expr::from_rule(eig_imag_pair)
            .context("Failed to build eig_imag expression in LEV")?
            .ok_or_else(|| anyhow::anyhow!("Expected eig_imag expression in LEV"))?;

        let eigvecs_pair = inner.next().context("Missing eigvecs parameter in LEV!")?;
        let eigvecs_expr = Expr::from_rule(eigvecs_pair)
            .context("Failed to build eigvecs expression in LEV")?
            .ok_or_else(|| anyhow::anyhow!("Expected eigvecs expression in LEV"))?;

        let n_pair = inner.next().context("Missing n parameter in LEV!")?;
        let n_expr = Expr::from_rule(n_pair)
            .context("Failed to build n expression in LEV")?
            .ok_or_else(|| anyhow::anyhow!("Expected n expression in LEV"))?;

        let alloc_dim_pair = inner
            .next()
            .context("Missing alloc_dim parameter in LEV!")?;
        let alloc_dim_expr = Expr::from_rule(alloc_dim_pair)
            .context("Failed to build alloc_dim expression in LEV")?
            .ok_or_else(|| anyhow::anyhow!("Expected alloc_dim expression in LEV"))?;

        Ok(Some(LevStatement {
            matrix_expr,
            eig_real_expr,
            eig_imag_expr,
            eigvecs_expr,
            n_expr,
            alloc_dim_expr,
        }))
    }
}

impl Transpile for LevStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let matrix_output = self
            .matrix_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling matrix in LEV".to_string()))?;
        requested_variables.extend(matrix_output.requested_variables.iter().cloned());

        let eig_real_output = self.eig_real_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling eig_real in LEV".to_string())
        })?;
        requested_variables.extend(eig_real_output.requested_variables.clone());

        let eig_imag_output = self.eig_imag_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling eig_imag in LEV".to_string())
        })?;
        requested_variables.extend(eig_imag_output.requested_variables.clone());

        let eigvecs_output = self.eigvecs_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling eigvecs in LEV".to_string())
        })?;
        requested_variables.extend(eigvecs_output.requested_variables.clone());

        let n_output = self
            .n_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling n in LEV".to_string()))?;
        requested_variables.extend(n_output.requested_variables.iter().cloned());

        let alloc_dim_output = self.alloc_dim_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling alloc_dim in LEV".to_string())
        })?;
        requested_variables.extend(alloc_dim_output.requested_variables.iter().cloned());

        fn make_lvalue(ser: &str, value_kind: ValueKind, rhs: &str) -> String {
            if value_kind == ValueKind::Owned {
                format!("{ser} = {rhs}")
            } else if let Some(bare) = ser.strip_prefix('&') {
                format!("{bare} = {rhs}")
            } else {
                format!("*{ser} = {rhs}")
            }
        }

        let eig_real_assign = make_lvalue(
            &eig_real_output.serialization,
            eig_real_output.value_kind,
            "rosy_lev_er",
        );
        let eig_imag_assign = make_lvalue(
            &eig_imag_output.serialization,
            eig_imag_output.value_kind,
            "rosy_lev_ei",
        );
        let eigvecs_assign = make_lvalue(
            &eigvecs_output.serialization,
            eigvecs_output.value_kind,
            "rosy_lev_v",
        );

        let serialization = format!(
            "{{ let (rosy_lev_er, rosy_lev_ei, rosy_lev_v) = rosy_lib::core::lev::rosy_lev({matrix}, {n} as usize, {alloc_dim} as usize)?; {eig_real_assign}; {eig_imag_assign}; {eigvecs_assign}; }}",
            matrix = matrix_output.as_ref(),
            n = n_output.as_value(),
            alloc_dim = alloc_dim_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for LevStatement {}
