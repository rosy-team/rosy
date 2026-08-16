//! # SCRLEN Statement
//!
//! Sets the amount of space scratch variables are allocated with.
//!
//! ## Syntax
//!
//! ```text
//! SCRLEN c;
//! ```
//!
//! ## Semantics in Rosy
//!
//! Scratch memory allocation is managed automatically by the Rust runtime.
//! SCRLEN is accepted for COSY compatibility but is a no-op.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/scrlen.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*, program::expressions::Expr, program::statements::SourceLocation, resolve::*,
    transpile::*,
};

/// AST node for `SCRLEN c;`.
#[derive(Debug)]
pub struct ScrlenStatement {
    pub size_expr: Expr,
}

impl FromRule for ScrlenStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::scrlen,
            "Expected `scrlen` rule when building SCRLEN statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let size_pair = inner.next().context("Missing size expression in SCRLEN!")?;
        let size_expr = Expr::from_rule(size_pair)
            .context("Failed to build size expression in SCRLEN")?
            .ok_or_else(|| anyhow::anyhow!("Expected size expression in SCRLEN"))?;

        Ok(Some(ScrlenStatement { size_expr }))
    }
}
impl TranspileableStatement for ScrlenStatement {
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
impl Transpile for ScrlenStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let size_output = self.size_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling size expression in SCRLEN".to_string(),
            )
        })?;
        requested_variables.extend(size_output.requested_variables.iter().cloned());

        // SCRLEN is a no-op in Rosy: scratch space is managed automatically by Rust.
        let serialization = format!(
            "{{ let _ = {}; /* SCRLEN: no-op in Rosy (scratch space managed automatically) */ }}",
            size_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
