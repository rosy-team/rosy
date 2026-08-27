//! # LINV Statement
//!
//! Inverts a quadratic matrix.
//!
//! ## Syntax
//!
//! ```text
//! LINV matrix inverse n alloc_dim error_flag;
//! ```
//!
//! - `matrix`     — input matrix (2D RE array, `Vec<Vec<f64>>`)
//! - `inverse`    — variable to receive the output inverse matrix
//! - `n`          — number of actual entries (RE, used as usize)
//! - `alloc_dim`  — allocation dimension (RE, used as usize)
//! - `error_flag` — variable to receive error code (0: no error, 132: singular)
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/linv.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableExpr,
        TranspileableStatement, ValueKind, add_context_to_all,
    },
};

/// AST node for `LINV matrix inverse n alloc_dim error_flag;`.
#[derive(Debug)]
pub struct LinvStatement {
    pub matrix_expr: Expr,
    pub inverse_expr: Expr,
    pub n_expr: Expr,
    pub alloc_dim_expr: Expr,
    pub error_flag_expr: Expr,
}

impl FromRule for LinvStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::linv,
            "Expected `linv` rule when building LINV statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let matrix_pair = inner.next().context("Missing matrix parameter in LINV!")?;
        let matrix_expr = Expr::from_rule(matrix_pair)
            .context("Failed to build matrix expression in LINV")?
            .ok_or_else(|| anyhow::anyhow!("Expected matrix expression in LINV"))?;

        let inverse_pair = inner.next().context("Missing inverse parameter in LINV!")?;
        let inverse_expr = Expr::from_rule(inverse_pair)
            .context("Failed to build inverse expression in LINV")?
            .ok_or_else(|| anyhow::anyhow!("Expected inverse expression in LINV"))?;

        let n_pair = inner.next().context("Missing n parameter in LINV!")?;
        let n_expr = Expr::from_rule(n_pair)
            .context("Failed to build n expression in LINV")?
            .ok_or_else(|| anyhow::anyhow!("Expected n expression in LINV"))?;

        let alloc_dim_pair = inner
            .next()
            .context("Missing alloc_dim parameter in LINV!")?;
        let alloc_dim_expr = Expr::from_rule(alloc_dim_pair)
            .context("Failed to build alloc_dim expression in LINV")?
            .ok_or_else(|| anyhow::anyhow!("Expected alloc_dim expression in LINV"))?;

        let error_flag_pair = inner
            .next()
            .context("Missing error_flag parameter in LINV!")?;
        let error_flag_expr = Expr::from_rule(error_flag_pair)
            .context("Failed to build error_flag expression in LINV")?
            .ok_or_else(|| anyhow::anyhow!("Expected error_flag expression in LINV"))?;

        Ok(Some(LinvStatement {
            matrix_expr,
            inverse_expr,
            n_expr,
            alloc_dim_expr,
            error_flag_expr,
        }))
    }
}

impl Transpile for LinvStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let matrix_output = self.matrix_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling matrix in LINV".to_string())
        })?;
        requested_variables.extend(matrix_output.requested_variables.iter().cloned());

        let inverse_output = self.inverse_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling inverse in LINV".to_string())
        })?;
        requested_variables.extend(inverse_output.requested_variables.clone());

        let n_output = self
            .n_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling n in LINV".to_string()))?;
        requested_variables.extend(n_output.requested_variables.iter().cloned());

        let alloc_dim_output = self.alloc_dim_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling alloc_dim in LINV".to_string())
        })?;
        requested_variables.extend(alloc_dim_output.requested_variables.iter().cloned());

        let error_flag_output = self.error_flag_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling error_flag in LINV".to_string())
        })?;
        requested_variables.extend(error_flag_output.requested_variables.clone());

        // Determine l-value assignment:
        // - Owned (Copy local): name is plain var → assign directly
        // - Ref with & prefix (non-Copy local): strip & → assign to bare name
        // - Ref without & (arg/higher): deref → *name = value
        fn make_lvalue(ser: &str, value_kind: ValueKind, rhs: &str) -> String {
            if value_kind == ValueKind::Owned {
                format!("{ser} = {rhs}")
            } else if let Some(bare) = ser.strip_prefix('&') {
                format!("{bare} = {rhs}")
            } else {
                format!("*{ser} = {rhs}")
            }
        }
        let inv_rhs = if self
            .inverse_expr
            .type_of(context)
            .map(|t| t.is_any())
            .unwrap_or(false)
        {
            "RosyValue::from(rosy_linv_inv).expect_arr2()?"
        } else {
            "rosy_linv_inv"
        };
        let inverse_assign = make_lvalue(
            &inverse_output.serialization,
            inverse_output.value_kind,
            inv_rhs,
        );
        let err_rhs = if self
            .error_flag_expr
            .type_of(context)
            .map(|t| t.is_any())
            .unwrap_or(false)
        {
            "RosyValue::from(rosy_linv_err)"
        } else {
            "rosy_linv_err"
        };
        let error_assign = make_lvalue(
            &error_flag_output.serialization,
            error_flag_output.value_kind,
            err_rhs,
        );

        let serialization = format!(
            "{{ let (rosy_linv_inv, rosy_linv_err) = rosy_lib::core::linv::rosy_linv({matrix}, rosy_as_usize(&({n})), rosy_as_usize(&({alloc_dim})))?; {inverse_assign}; {error_assign}; }}",
            matrix = matrix_output.as_ref(),
            n = n_output.as_value(),
            alloc_dim = alloc_dim_output.as_value(),
            inverse_assign = inverse_assign,
            error_assign = error_assign,
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for LinvStatement {}
