//! # SAVE Statement
//!
//! Saves compiled code to a `.bin` file for later inclusion.
//!
//! ## Syntax
//!
//! ```text
//! SAVE 'filename' ;
//! ```
//!
//! ## Semantics in Rosy
//!
//! COSY's SAVE writes compiled bytecode for later INCLUDE. Since Rosy
//! transpiles to native Rust, there is no bytecode to save. SAVE is accepted
//! for COSY compatibility but emits a compile-time warning and generates no
//! runtime code.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/io/save.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*, program::expressions::Expr, program::statements::SourceLocation, resolve::*,
    transpile::*,
};

/// AST node for `SAVE <expr> ;`.
#[derive(Debug)]
pub struct SaveStatement {
    pub filename_expr: Expr,
}

impl FromRule for SaveStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::save_stmt,
            "Expected `save_stmt` rule when building SAVE statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let filename_pair = inner
            .next()
            .context("Missing filename expression in SAVE!")?;
        let filename_expr = Expr::from_rule(filename_pair)
            .context("Failed to build filename expression in SAVE")?
            .ok_or_else(|| anyhow::anyhow!("Expected filename expression in SAVE"))?;

        Ok(Some(SaveStatement { filename_expr }))
    }
}
impl TranspileableStatement for SaveStatement {
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
impl Transpile for SaveStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let filename_output = self.filename_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling filename expression in SAVE".to_string(),
            )
        })?;
        requested_variables.extend(filename_output.requested_variables.iter().cloned());

        // Emit a compile-time warning
        eprintln!(
            "warning: SAVE statement is a no-op in Rosy (COSY bytecode saving is not applicable when transpiling to native Rust)"
        );

        // Evaluate the expression (for side effects / variable tracking) but discard it
        let serialization = format!(
            "{{ let _ = {}; /* SAVE: no-op in Rosy (no bytecode to save) */ }}",
            filename_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
