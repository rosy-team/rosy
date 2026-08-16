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
    program::{expressions::Expr, statements::SourceLocation},
    resolve::{ScopeContext, TypeResolver},
    transpile::{
        InferenceEdgeResult, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableStatement, TypeHydrationResult, TypeslotDeclarationResult, add_context_to_all,
    },
};

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

impl TranspileableStatement for DafloStatement {
    fn register_typeslot_declaration(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> TypeslotDeclarationResult {
        TypeslotDeclarationResult::NotAVarFuncOrProcedureDecl
    }
    fn wire_inference_edges(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> InferenceEdgeResult {
        InferenceEdgeResult::NoEdges
    }
    fn hydrate_resolved_types(
        &mut self,
        _resolver: &TypeResolver,
        _current_scope: &[String],
    ) -> TypeHydrationResult {
        TypeHydrationResult::NothingToHydrate
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
            "rosy_lib::core::da_ops::rosy_daflo({}, {}, {result_ref}, {} as usize)?;",
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
