//! # MBLOCK Statement
//!
//! Transforms a quadratic matrix to block-diagonal form.
//!
//! ## Syntax
//!
//! ```text
//! MBLOCK matrix transform inverse alloc_dim n;
//! ```
//!
//! - `matrix`    — input matrix (RE ** 2)
//! - `transform` — variable for transformation matrix (RE ** 2)
//! - `inverse`   — variable for inverse transformation matrix (RE ** 2)
//! - `alloc_dim` — allocation dimension (RE, used as usize)
//! - `n`         — number of actual entries (RE, used as usize)
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/mblock.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement, ValueKind, add_context_to_all},
};

#[derive(Debug)]
pub struct MblockStatement {
    pub matrix_expr: Expr,
    pub transform_expr: Expr,
    pub inverse_expr: Expr,
    pub alloc_dim_expr: Expr,
    pub n_expr: Expr,
}

impl FromRule for MblockStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::mblock,
            "Expected `mblock` rule when building MBLOCK statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let matrix_pair = inner
            .next()
            .context("Missing matrix parameter in MBLOCK!")?;
        let matrix_expr = Expr::from_rule(matrix_pair)
            .context("Failed to build matrix expression in MBLOCK")?
            .ok_or_else(|| anyhow::anyhow!("Expected matrix expression in MBLOCK"))?;

        let transform_pair = inner
            .next()
            .context("Missing transform parameter in MBLOCK!")?;
        let transform_expr = Expr::from_rule(transform_pair)
            .context("Failed to build transform expression in MBLOCK")?
            .ok_or_else(|| anyhow::anyhow!("Expected transform expression in MBLOCK"))?;

        let inverse_pair = inner
            .next()
            .context("Missing inverse parameter in MBLOCK!")?;
        let inverse_expr = Expr::from_rule(inverse_pair)
            .context("Failed to build inverse expression in MBLOCK")?
            .ok_or_else(|| anyhow::anyhow!("Expected inverse expression in MBLOCK"))?;

        let alloc_dim_pair = inner
            .next()
            .context("Missing alloc_dim parameter in MBLOCK!")?;
        let alloc_dim_expr = Expr::from_rule(alloc_dim_pair)
            .context("Failed to build alloc_dim expression in MBLOCK")?
            .ok_or_else(|| anyhow::anyhow!("Expected alloc_dim expression in MBLOCK"))?;

        let n_pair = inner.next().context("Missing n parameter in MBLOCK!")?;
        let n_expr = Expr::from_rule(n_pair)
            .context("Failed to build n expression in MBLOCK")?
            .ok_or_else(|| anyhow::anyhow!("Expected n expression in MBLOCK"))?;

        Ok(Some(MblockStatement {
            matrix_expr,
            transform_expr,
            inverse_expr,
            alloc_dim_expr,
            n_expr,
        }))
    }
}


impl Transpile for MblockStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let matrix_output = self.matrix_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling matrix in MBLOCK".to_string())
        })?;
        requested_variables.extend(matrix_output.requested_variables.iter().cloned());

        let transform_output = self.transform_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling transform in MBLOCK".to_string())
        })?;
        requested_variables.extend(transform_output.requested_variables.clone());

        let inverse_output = self.inverse_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling inverse in MBLOCK".to_string())
        })?;
        requested_variables.extend(inverse_output.requested_variables.clone());

        let alloc_dim_output = self.alloc_dim_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling alloc_dim in MBLOCK".to_string())
        })?;
        requested_variables.extend(alloc_dim_output.requested_variables.iter().cloned());

        let n_output = self
            .n_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling n in MBLOCK".to_string()))?;
        requested_variables.extend(n_output.requested_variables.iter().cloned());

        fn make_lvalue(ser: &str, value_kind: ValueKind, rhs: &str) -> String {
            if value_kind == ValueKind::Owned {
                format!("{ser} = {rhs}")
            } else if let Some(bare) = ser.strip_prefix('&') {
                format!("{bare} = {rhs}")
            } else {
                format!("*{ser} = {rhs}")
            }
        }

        let transform_assign = make_lvalue(
            &transform_output.serialization,
            transform_output.value_kind,
            "rosy_mblock_t",
        );
        let inverse_assign = make_lvalue(
            &inverse_output.serialization,
            inverse_output.value_kind,
            "rosy_mblock_ti",
        );

        let serialization = format!(
            "{{ let (rosy_mblock_t, rosy_mblock_ti) = rosy_lib::core::mblock::rosy_mblock({matrix}, {n} as usize, {alloc_dim} as usize)?; {transform_assign}; {inverse_assign}; }}",
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

impl TranspileableStatement for MblockStatement {}
