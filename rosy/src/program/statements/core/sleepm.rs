//! # SLEEPM Statement
//!
//! Suspends program execution for a given duration in milliseconds.
//!
//! ## Syntax
//!
//! ```text
//! SLEEPM c;
//! ```
//!
//! ## Semantics in Rosy
//!
//! Transpiles to `std::thread::sleep(std::time::Duration::from_millis(...))`.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/sleepm.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*, program::expressions::Expr, program::statements::SourceLocation, resolve::*,
    transpile::*,
};

/// AST node for `SLEEPM c;`.
#[derive(Debug)]
pub struct SleepmStatement {
    pub duration_expr: Expr,
}

impl FromRule for SleepmStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::sleepm,
            "Expected `sleepm` rule when building SLEEPM statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let duration_pair = inner
            .next()
            .context("Missing duration expression in SLEEPM!")?;
        let duration_expr = Expr::from_rule(duration_pair)
            .context("Failed to build duration expression in SLEEPM")?
            .ok_or_else(|| anyhow::anyhow!("Expected duration expression in SLEEPM"))?;

        Ok(Some(SleepmStatement { duration_expr }))
    }
}
impl TranspileableStatement for SleepmStatement {
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
impl Transpile for SleepmStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let duration_output = self.duration_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling duration expression in SLEEPM".to_string(),
            )
        })?;
        requested_variables.extend(duration_output.requested_variables.iter().cloned());

        let serialization = format!(
            "std::thread::sleep(std::time::Duration::from_millis({} as u64));",
            duration_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
