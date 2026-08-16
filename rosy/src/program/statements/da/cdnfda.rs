//! # CDNFDA Statement (Complex DA Normal Form — Cj Operator)
//!
//! Applies the Cj operator for non-symplectic normal forms where eigenvalues
//! have |lambda| != 1. Uses separate moduli and arguments of eigenvalues.
//!
//! ## Syntax
//!
//! ```text
//! CDNFDA input moduli arguments coord total epsilon result;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/cdnfda.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement, add_context_to_all},
};

/// AST node for `CDNFDA input moduli arguments coord total epsilon result;`.
#[derive(Debug)]
pub struct CdnfdaStatement {
    pub input: Expr,
    pub moduli: Expr,
    pub arguments: Expr,
    pub coord: Expr,
    pub total: Expr,
    pub epsilon: Expr,
    pub result: Expr,
}

impl FromRule for CdnfdaStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::cdnfda,
            "Expected `cdnfda` rule when building CDNFDA statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();
        let fields = [
            "input",
            "moduli",
            "arguments",
            "coord",
            "total",
            "epsilon",
            "result",
        ];
        let mut exprs = Vec::new();
        for name in &fields {
            let p = inner
                .next()
                .context(format!("Missing {} in CDNFDA", name))?;
            let e = Expr::from_rule(p)
                .context(format!("Failed to build {} expression in CDNFDA", name))?
                .ok_or_else(|| anyhow::anyhow!("Expected {} expression in CDNFDA", name))?;
            exprs.push(e);
        }

        Ok(Some(CdnfdaStatement {
            input: exprs.remove(0),
            moduli: exprs.remove(0),
            arguments: exprs.remove(0),
            coord: exprs.remove(0),
            total: exprs.remove(0),
            epsilon: exprs.remove(0),
            result: exprs.remove(0),
        }))
    }
}


impl Transpile for CdnfdaStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let input_o = self.input.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling input in CDNFDA".to_string())
        })?;
        requested_variables.extend(input_o.requested_variables.iter().cloned());

        let mod_o = self.moduli.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling moduli in CDNFDA".to_string())
        })?;
        requested_variables.extend(mod_o.requested_variables.iter().cloned());

        let arg_o = self.arguments.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling arguments in CDNFDA".to_string())
        })?;
        requested_variables.extend(arg_o.requested_variables.iter().cloned());

        let coord_o = self.coord.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling coord in CDNFDA".to_string())
        })?;
        requested_variables.extend(coord_o.requested_variables.iter().cloned());

        let total_o = self.total.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling total in CDNFDA".to_string())
        })?;
        requested_variables.extend(total_o.requested_variables.iter().cloned());

        let eps_o = self.epsilon.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling epsilon in CDNFDA".to_string())
        })?;
        requested_variables.extend(eps_o.requested_variables.iter().cloned());

        let result_o = self.result.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in CDNFDA".to_string())
        })?;
        requested_variables.extend(result_o.requested_variables.iter().cloned());

        let result_ref = result_o.as_mut_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_cdnfda({}, {}, {}, {} as usize, {} as usize, {}, {})?;",
            input_o.as_ref(),
            mod_o.as_ref(),
            arg_o.as_ref(),
            coord_o.as_value(),
            total_o.as_value(),
            eps_o.as_value(),
            result_ref,
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for CdnfdaStatement {}
