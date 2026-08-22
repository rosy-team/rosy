//! # LDET Statement
//!
//! Computes the determinant of a matrix.
//!
//! ## Syntax
//!
//! ```text
//! LDET matrix n alloc_dim result ;
//! ```
//!
//! Arguments:
//! 1. `matrix`    — input matrix (`RE ** 2`, i.e. `Vec<Vec<f64>>`)
//! 2. `n`         — number of actual rows/columns (RE, used as usize)
//! 3. `alloc_dim` — allocation dimension (RE, used as usize)
//! 4. `result`    — variable to receive the determinant (RE, must be a variable identifier)
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/ldet.rosy"))]
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

/// AST node for `LDET matrix n alloc_dim result ;`.
#[derive(Debug)]
pub struct LdetStatement {
    pub matrix_expr: Expr,
    pub n_expr: Expr,
    pub alloc_dim_expr: Expr,
    pub result_expr: Expr,
}

impl FromRule for LdetStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::ldet,
            "Expected `ldet` rule when building LDET statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let matrix_pair = inner.next().context("Missing matrix parameter in LDET!")?;
        let matrix_expr = Expr::from_rule(matrix_pair)
            .context("Failed to build matrix expression in LDET")?
            .ok_or_else(|| anyhow::anyhow!("Expected matrix expression in LDET"))?;

        let n_pair = inner.next().context("Missing n parameter in LDET!")?;
        let n_expr = Expr::from_rule(n_pair)
            .context("Failed to build n expression in LDET")?
            .ok_or_else(|| anyhow::anyhow!("Expected n expression in LDET"))?;

        let alloc_dim_pair = inner
            .next()
            .context("Missing alloc_dim parameter in LDET!")?;
        let alloc_dim_expr = Expr::from_rule(alloc_dim_pair)
            .context("Failed to build alloc_dim expression in LDET")?
            .ok_or_else(|| anyhow::anyhow!("Expected alloc_dim expression in LDET"))?;

        let result_pair = inner.next().context("Missing result parameter in LDET!")?;
        let result_expr = Expr::from_rule(result_pair)
            .context("Failed to build result expression in LDET")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in LDET"))?;

        Ok(Some(LdetStatement {
            matrix_expr,
            n_expr,
            alloc_dim_expr,
            result_expr,
        }))
    }
}

impl Transpile for LdetStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let matrix_output = self.matrix_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling matrix in LDET".to_string())
        })?;
        requested_variables.extend(matrix_output.requested_variables.iter().cloned());

        let n_output = self
            .n_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling n in LDET".to_string()))?;
        requested_variables.extend(n_output.requested_variables.iter().cloned());

        let alloc_dim_output = self.alloc_dim_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling alloc_dim in LDET".to_string())
        })?;
        requested_variables.extend(alloc_dim_output.requested_variables.iter().cloned());

        let result_output = self.result_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in LDET".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.clone());

        // Determine l-value assignment:
        // - Owned (Copy local): assign directly
        // - Ref with & prefix (non-Copy local): strip & → bare name
        // - Ref without & (arg/higher): deref → *name = value
        let matrix_ref = matrix_output.as_ref();
        let n_val = n_output.as_value();
        let alloc_val = alloc_dim_output.as_value();
        let mut rhs = format!(
            "rosy_lib::core::ldet::rosy_ldet({matrix_ref}, rosy_as_usize(&({n_val})), rosy_as_usize(&({alloc_val})))?"
        );
        if self
            .result_expr
            .type_of(context)
            .map(|t| t.is_any())
            .unwrap_or(false)
        {
            rhs = format!("RosyValue::from({rhs})");
        }
        let result_assign = if result_output.value_kind == ValueKind::Owned {
            format!("{} = {rhs}", result_output.serialization)
        } else if let Some(bare) = result_output.serialization.strip_prefix('&') {
            format!("{bare} = {rhs}")
        } else {
            format!("*{} = {rhs}", result_output.serialization)
        };

        let serialization = format!("{result_assign};");

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for LdetStatement {}
