//! # DATRN Statement
//!
//! Transforms the independent variables of a DA vector by affine maps:
//!   x_i -> a_i * x_i + c_i  for i = m1, ..., m2
//!
//! ## Syntax
//!
//! ```text
//! DATRN input scales shifts m1 m2 output;
//! ```
//!
//! Arguments:
//! 1. `input`  (DA vector, read)  - source DA array
//! 2. `scales` (VE, read)         - array of scale factors a_i
//! 3. `shifts` (VE, read)         - array of translation factors c_i
//! 4. `m1`     (RE, read)         - start variable index (1-based)
//! 5. `m2`     (RE, read)         - end variable index (1-based, inclusive)
//! 6. `output` (DA vector, write) - result DA array
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/datrn.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement, add_context_to_all},
};

/// AST node for `DATRN input scales shifts m1 m2 output;`.
#[derive(Debug)]
pub struct DatrnStatement {
    pub input_expr: Expr,
    pub scales_expr: Expr,
    pub shifts_expr: Expr,
    pub m1_expr: Expr,
    pub m2_expr: Expr,
    pub output_expr: Expr,
}

impl FromRule for DatrnStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::datrn,
            "Expected `datrn` rule when building DATRN statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let input_pair = inner.next().context("Missing input parameter in DATRN!")?;
        let input_expr = Expr::from_rule(input_pair)
            .context("Failed to build input expression in DATRN")?
            .ok_or_else(|| anyhow::anyhow!("Expected input expression in DATRN"))?;

        let scales_pair = inner.next().context("Missing scales parameter in DATRN!")?;
        let scales_expr = Expr::from_rule(scales_pair)
            .context("Failed to build scales expression in DATRN")?
            .ok_or_else(|| anyhow::anyhow!("Expected scales expression in DATRN"))?;

        let shifts_pair = inner.next().context("Missing shifts parameter in DATRN!")?;
        let shifts_expr = Expr::from_rule(shifts_pair)
            .context("Failed to build shifts expression in DATRN")?
            .ok_or_else(|| anyhow::anyhow!("Expected shifts expression in DATRN"))?;

        let m1_pair = inner.next().context("Missing m1 parameter in DATRN!")?;
        let m1_expr = Expr::from_rule(m1_pair)
            .context("Failed to build m1 expression in DATRN")?
            .ok_or_else(|| anyhow::anyhow!("Expected m1 expression in DATRN"))?;

        let m2_pair = inner.next().context("Missing m2 parameter in DATRN!")?;
        let m2_expr = Expr::from_rule(m2_pair)
            .context("Failed to build m2 expression in DATRN")?
            .ok_or_else(|| anyhow::anyhow!("Expected m2 expression in DATRN"))?;

        let output_pair = inner.next().context("Missing output parameter in DATRN!")?;
        let output_expr = Expr::from_rule(output_pair)
            .context("Failed to build output expression in DATRN")?
            .ok_or_else(|| anyhow::anyhow!("Expected output expression in DATRN"))?;

        Ok(Some(DatrnStatement {
            input_expr,
            scales_expr,
            shifts_expr,
            m1_expr,
            m2_expr,
            output_expr,
        }))
    }
}


impl Transpile for DatrnStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let input_output = self.input_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling input in DATRN".to_string())
        })?;
        requested_variables.extend(input_output.requested_variables.iter().cloned());

        let scales_output = self.scales_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling scales in DATRN".to_string())
        })?;
        requested_variables.extend(scales_output.requested_variables.iter().cloned());

        let shifts_output = self.shifts_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling shifts in DATRN".to_string())
        })?;
        requested_variables.extend(shifts_output.requested_variables.iter().cloned());

        let m1_output = self
            .m1_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling m1 in DATRN".to_string()))?;
        requested_variables.extend(m1_output.requested_variables.iter().cloned());

        let m2_output = self
            .m2_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling m2 in DATRN".to_string()))?;
        requested_variables.extend(m2_output.requested_variables.iter().cloned());

        let out_output = self.output_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling output in DATRN".to_string())
        })?;
        requested_variables.extend(out_output.requested_variables.clone());

        let serialization = format!(
            "rosy_lib::core::daprv::rosy_datrn({}, {}, {}, {} as usize, {} as usize, {})?;",
            input_output.as_ref(),
            scales_output.as_ref(),
            shifts_output.as_ref(),
            m1_output.as_value(),
            m2_output.as_value(),
            out_output.as_mut_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DatrnStatement {}
