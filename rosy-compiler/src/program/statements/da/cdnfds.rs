//! # CDNFDS Statement (Complex DA Normal Form — Sj Spin Operator)
//!
//! Applies the Sj operator for spin normal forms. Like CDNFDA but the target
//! eigenvalue is lambda_spin = exp(i * spin_argument) for spin dynamics.
//!
//! ## Syntax
//!
//! ```text
//! CDNFDS input moduli arguments spin_arg total epsilon result;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/cdnfds.rosy"))]
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

/// AST node for `CDNFDS input moduli arguments spin_arg total epsilon result;`.
#[derive(Debug)]
pub struct CdnfdsStatement {
    pub input: Expr,
    pub moduli: Expr,
    pub arguments: Expr,
    pub spin_arg: Expr,
    pub total: Expr,
    pub epsilon: Expr,
    pub result: Expr,
}

impl FromRule for CdnfdsStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::cdnfds,
            "Expected `cdnfds` rule when building CDNFDS statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();
        let fields = [
            "input",
            "moduli",
            "arguments",
            "spin_arg",
            "total",
            "epsilon",
            "result",
        ];
        let mut exprs = Vec::new();
        for name in &fields {
            let p = inner
                .next()
                .context(format!("Missing {} in CDNFDS", name))?;
            let e = Expr::from_rule(p)
                .context(format!("Failed to build {} expression in CDNFDS", name))?
                .ok_or_else(|| anyhow::anyhow!("Expected {} expression in CDNFDS", name))?;
            exprs.push(e);
        }

        Ok(Some(CdnfdsStatement {
            input: exprs.remove(0),
            moduli: exprs.remove(0),
            arguments: exprs.remove(0),
            spin_arg: exprs.remove(0),
            total: exprs.remove(0),
            epsilon: exprs.remove(0),
            result: exprs.remove(0),
        }))
    }
}

impl Transpile for CdnfdsStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let input_o = self.input.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling input in CDNFDS".to_string())
        })?;
        requested_variables.extend(input_o.requested_variables.iter().cloned());

        let mod_o = self.moduli.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling moduli in CDNFDS".to_string())
        })?;
        requested_variables.extend(mod_o.requested_variables.iter().cloned());

        let arg_o = self.arguments.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling arguments in CDNFDS".to_string())
        })?;
        requested_variables.extend(arg_o.requested_variables.iter().cloned());

        let spin_o = self.spin_arg.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling spin_arg in CDNFDS".to_string())
        })?;
        requested_variables.extend(spin_o.requested_variables.iter().cloned());

        let total_o = self.total.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling total in CDNFDS".to_string())
        })?;
        requested_variables.extend(total_o.requested_variables.iter().cloned());

        let eps_o = self.epsilon.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling epsilon in CDNFDS".to_string())
        })?;
        requested_variables.extend(eps_o.requested_variables.iter().cloned());

        let result_o = self.result.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in CDNFDS".to_string())
        })?;
        requested_variables.extend(result_o.requested_variables.iter().cloned());

        let result_ref = result_o.as_mut_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_cdnfds({}, {}, {}, {}, rosy_as_usize(&({})), {}, {})?;",
            input_o.as_ref(),
            mod_o.as_ref(),
            arg_o.as_ref(),
            spin_o.as_value(),
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

impl TranspileableStatement for CdnfdsStatement {}
