//! # CDNF Statement (Complex DA Normal Form — Homological Equation)
//!
//! Applies the resolvent operator 1/(1-exp(:f2:)) to a CD vector in Floquet
//! variables, solving the homological equation for normal form analysis.
//! Resonant terms (small denominators or matching the resonance list) are zeroed.
//!
//! ## Syntax
//!
//! ```text
//! CDNF input tune1 tune2 tune3 resonances res_dims n_res result;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/cdnf.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement, add_context_to_all},
};

/// AST node for `CDNF input t1 t2 t3 resonances res_dims n_res result;`.
#[derive(Debug)]
pub struct CdnfStatement {
    pub input: Expr,
    pub tune1: Expr,
    pub tune2: Expr,
    pub tune3: Expr,
    pub resonances: Expr,
    pub res_dims: Expr,
    pub n_res: Expr,
    pub result: Expr,
}

impl FromRule for CdnfStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::cdnf,
            "Expected `cdnf` rule when building CDNF statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let input = Expr::from_rule(inner.next().context("Missing input in CDNF")?)
            .context("Failed to build input expression in CDNF")?
            .ok_or_else(|| anyhow::anyhow!("Expected input expression in CDNF"))?;

        let tune1 = Expr::from_rule(inner.next().context("Missing tune1 in CDNF")?)
            .context("Failed to build tune1 expression in CDNF")?
            .ok_or_else(|| anyhow::anyhow!("Expected tune1 expression in CDNF"))?;

        let tune2 = Expr::from_rule(inner.next().context("Missing tune2 in CDNF")?)
            .context("Failed to build tune2 expression in CDNF")?
            .ok_or_else(|| anyhow::anyhow!("Expected tune2 expression in CDNF"))?;

        let tune3 = Expr::from_rule(inner.next().context("Missing tune3 in CDNF")?)
            .context("Failed to build tune3 expression in CDNF")?
            .ok_or_else(|| anyhow::anyhow!("Expected tune3 expression in CDNF"))?;

        let resonances = Expr::from_rule(inner.next().context("Missing resonances in CDNF")?)
            .context("Failed to build resonances expression in CDNF")?
            .ok_or_else(|| anyhow::anyhow!("Expected resonances expression in CDNF"))?;

        let res_dims = Expr::from_rule(inner.next().context("Missing res_dims in CDNF")?)
            .context("Failed to build res_dims expression in CDNF")?
            .ok_or_else(|| anyhow::anyhow!("Expected res_dims expression in CDNF"))?;

        let n_res = Expr::from_rule(inner.next().context("Missing n_res in CDNF")?)
            .context("Failed to build n_res expression in CDNF")?
            .ok_or_else(|| anyhow::anyhow!("Expected n_res expression in CDNF"))?;

        let result = Expr::from_rule(inner.next().context("Missing result in CDNF")?)
            .context("Failed to build result expression in CDNF")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in CDNF"))?;

        Ok(Some(CdnfStatement {
            input,
            tune1,
            tune2,
            tune3,
            resonances,
            res_dims,
            n_res,
            result,
        }))
    }
}


impl Transpile for CdnfStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let input_output = self
            .input
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling input in CDNF".to_string()))?;
        requested_variables.extend(input_output.requested_variables.iter().cloned());

        let t1_output = self
            .tune1
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling tune1 in CDNF".to_string()))?;
        requested_variables.extend(t1_output.requested_variables.iter().cloned());

        let t2_output = self
            .tune2
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling tune2 in CDNF".to_string()))?;
        requested_variables.extend(t2_output.requested_variables.iter().cloned());

        let t3_output = self
            .tune3
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling tune3 in CDNF".to_string()))?;
        requested_variables.extend(t3_output.requested_variables.iter().cloned());

        let res_output = self.resonances.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling resonances in CDNF".to_string())
        })?;
        requested_variables.extend(res_output.requested_variables.iter().cloned());

        let dims_output = self.res_dims.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling res_dims in CDNF".to_string())
        })?;
        requested_variables.extend(dims_output.requested_variables.iter().cloned());

        let nres_output = self
            .n_res
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling n_res in CDNF".to_string()))?;
        requested_variables.extend(nres_output.requested_variables.iter().cloned());

        let result_output = self.result.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in CDNF".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let result_ref = result_output.as_mut_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_cdnf({}, {}, {}, {}, {}, {}, {} as usize, {})?;",
            input_output.as_ref(),
            t1_output.as_value(),
            t2_output.as_value(),
            t3_output.as_value(),
            res_output.as_ref(),
            dims_output.as_ref(),
            nres_output.as_value(),
            result_ref,
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for CdnfStatement {}
