//! # RANSEED Statement
//!
//! Sets the global random number generator seed.
//!
//! ## Syntax
//!
//! ```text
//! RANSEED seed_expr;
//! ```
//!
//! - If `seed_expr` evaluates to a **negative** value, the RNG is reseeded
//!   from system entropy (non-deterministic).
//! - If `seed_expr` evaluates to a **non-negative** value, it is truncated
//!   to an unsigned integer and used as a deterministic seed.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/ranseed.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};

use crate::{
    ast::*,
    program::{expressions::Expr, statements::SourceLocation},
    resolve::{ScopeContext, TypeResolver},
    transpile::{
        InferenceEdgeResult, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableStatement, TypeHydrationResult, TypeslotDeclarationResult,
    },
};

/// AST node for the `RANSEED seed;` statement.
#[derive(Debug)]
pub struct RanseedStatement {
    pub seed: Expr,
}

impl FromRule for RanseedStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::ranseed,
            "Expected `ranseed` rule when building RANSEED statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let seed_pair = inner
            .next()
            .context("Missing seed parameter in RANSEED statement!")?;
        let seed_expr = Expr::from_rule(seed_pair)
            .context("Failed to build seed expression in RANSEED statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected expression for seed in RANSEED statement"))?;

        Ok(Some(RanseedStatement { seed: seed_expr }))
    }
}

impl TranspileableStatement for RanseedStatement {
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

impl Transpile for RanseedStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let seed_output = self.seed.transpile(context).map_err(|errs| {
            errs.into_iter()
                .map(|e| e.context("...while transpiling seed expression in RANSEED"))
                .collect::<Vec<_>>()
        })?;

        let serialization = format!(
            "rosy_lib::core::rng::set_rng_seed({} as f64);",
            seed_output.as_value()
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables: seed_output.requested_variables,
            ..Default::default()
        })
    }
}
