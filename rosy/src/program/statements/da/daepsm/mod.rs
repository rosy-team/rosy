//! # DAEPSM Statement (DA Epsilon Getter)
//!
//! Returns the current garbage collection tolerance (cutoff threshold) for
//! coefficients of DA and CD vectors into a variable.
//!
//! ## Syntax
//!
//! ```text
//! DAEPSM v;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/daepsm.rosy"))]
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

/// AST node for the `DAEPSM v;` DA epsilon getter statement.
#[derive(Debug)]
pub struct DaepsmStatement {
    pub result: Expr,
}

impl FromRule for DaepsmStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::daepsm,
            "Expected `daepsm` rule when building DAEPSM statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let result_pair = inner
            .next()
            .context("Missing result variable in DAEPSM statement!")?;
        let result = Expr::from_rule(result_pair)
            .context("Failed to build result expression in DAEPSM statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DAEPSM statement"))?;

        Ok(Some(DaepsmStatement { result }))
    }
}

impl TranspileableStatement for DaepsmStatement {
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

impl Transpile for DaepsmStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let result_output = self.result.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DAEPSM".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let result_ref = result_output.as_mut_ref();

        let serialization = format!("*{result_ref} = taylor::get_config()?.epsilon;");

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
