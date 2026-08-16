//! # DASGN Statement (DA Sign / Negate)
//!
//! Negates all coefficients of a DA vector array (sign flip).
//!
//! ## Syntax
//!
//! ```text
//! DASGN da_var;
//! ```
//!
//! Arguments:
//! 1. `da_var` (DA array, in/out) — DA vector to negate in place
//!
//! > **COSY note**: In COSY INFINITY, `DASGN` has different semantics — it flips signs
//! > to make the first Ns linear coefficients positive, returning the sign array and result.
//! > It takes 4 arguments `(DA, Ns, signs_out, result)`. Rosy's form negates all coefficients.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dasgn.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
        add_context_to_all,
    },
};

/// AST node for `DASGN da_var;`.
#[derive(Debug)]
pub struct DasgnStatement {
    pub da_expr: Expr,
}

impl FromRule for DasgnStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dasgn,
            "Expected `dasgn` rule when building DASGN statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_pair = inner.next().context("Missing da_var parameter in DASGN!")?;
        let da_expr = Expr::from_rule(da_pair)
            .context("Failed to build da_var expression in DASGN")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DASGN"))?;

        Ok(Some(DasgnStatement { da_expr }))
    }
}

impl Transpile for DasgnStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let da_output = self.da_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DASGN".to_string())
        })?;

        let da_mut = da_output.as_mut_ref();

        let serialization = format!("rosy_lib::core::da_ops::rosy_dasgn({})?;", da_mut,);

        Ok(TranspilationOutput {
            serialization,
            requested_variables: da_output.requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DasgnStatement {}
