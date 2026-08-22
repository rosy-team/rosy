//! # DAFLO Statement (DA Flow)
//!
//! Computes the DA representation of the flow of x' = f(x) for time step 1
//! to nearly machine accuracy via iterated Lie series: exp(L_f)(ic).
//!
//! Arguments: `rhs` (array of DA right-hand sides), `ic` (initial condition),
//! `result` (output), `dim` (dimension of f).
//!
//! ## Syntax
//!
//! ```text
//! DAFLO rhs ic result dim;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/daflo.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    resolve::{ExprRecipe, ResolutionRule, ScopeContext, TypeResolver},
    syntax_config,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableExpr,
        TranspileableStatement, add_context_to_all,
    },
};
use rosy_lib::RosyType;

/// AST node for the `DAFLO rhs ic result dim;` ODE flow statement.
#[derive(Debug)]
pub struct DafloStatement {
    pub rhs: Expr,
    pub ic: Expr,
    pub result: Expr,
    pub dim: Expr,
}

impl FromRule for DafloStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::daflo,
            "Expected `daflo` rule when building DAFLO statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let rhs = Expr::from_rule(inner.next().context("Missing rhs in DAFLO")?)
            .context("Failed to build rhs expression in DAFLO")?
            .ok_or_else(|| anyhow::anyhow!("Expected rhs expression in DAFLO"))?;

        let ic = Expr::from_rule(inner.next().context("Missing ic in DAFLO")?)
            .context("Failed to build ic expression in DAFLO")?
            .ok_or_else(|| anyhow::anyhow!("Expected ic expression in DAFLO"))?;

        let result = Expr::from_rule(inner.next().context("Missing result in DAFLO")?)
            .context("Failed to build result expression in DAFLO")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DAFLO"))?;

        let dim = Expr::from_rule(inner.next().context("Missing dim in DAFLO")?)
            .context("Failed to build dim expression in DAFLO")?
            .ok_or_else(|| anyhow::anyhow!("Expected dim expression in DAFLO"))?;

        Ok(Some(DafloStatement {
            rhs,
            ic,
            result,
            dim,
        }))
    }
}

impl Transpile for DafloStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let rhs_output = self
            .rhs
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling rhs in DAFLO".to_string()))?;
        requested_variables.extend(rhs_output.requested_variables.iter().cloned());

        let ic_output = self
            .ic
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling ic in DAFLO".to_string()))?;
        requested_variables.extend(ic_output.requested_variables.iter().cloned());

        let result_output = self.result.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DAFLO".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let dim_output = self
            .dim
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling dim in DAFLO".to_string()))?;
        requested_variables.extend(dim_output.requested_variables.iter().cloned());

        let result_ref = result_output.as_mut_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_daflo({}, {}, {result_ref}, rosy_as_usize(&({})))?;",
            rhs_output.as_ref(),
            ic_output.as_ref(),
            dim_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DafloStatement {
    fn wire_inference_edges(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        _source_location: crate::program::statements::SourceLocation,
    ) -> Option<Result<()>> {
        let Some(name) = self.result.as_bare_variable_name() else {
            return Some(Ok(()));
        };
        let Some(slot) = ctx.variables.get(name).cloned() else {
            return Some(Ok(()));
        };
        if let Some(node) = resolver.nodes.get_mut(&slot) {
            if syntax_config::is_cosy_syntax() {
                node.rule = ResolutionRule::InferredFrom {
                    recipe: ExprRecipe::Literal(RosyType::ANY()),
                    reason: "DAFLO dest (COSY cell)".into(),
                };
                node.resolved = Some(RosyType::ANY());
                node.depends_on.clear();
            } else if node.resolved.is_none() {
                node.rule = ResolutionRule::InferredFrom {
                    recipe: ExprRecipe::Literal(RosyType::DA()),
                    reason: "DAFLO dest".into(),
                };
            }
        }
        Some(Ok(()))
    }
}
