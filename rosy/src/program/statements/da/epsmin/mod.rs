//! # EPSMIN Statement (Machine Underflow Threshold)
//!
//! Returns the underflow threshold — the smallest positive number representable
//! on the system (`f64::MIN_POSITIVE`, equivalent to `2.225e-308`).
//!
//! ## Syntax
//!
//! ```text
//! EPSMIN v;
//! ```
//!
//! ## Rosy Example
//! ```text
#![doc = include_str!("test.rosy")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("rosy_output.txt")]
//! ```
//! ## COSY INFINITY Example
//! ```text
#![doc = include_str!("test.fox")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("cosy_output.txt")]
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

/// AST node for the `EPSMIN v;` machine underflow threshold statement.
#[derive(Debug)]
pub struct EpsminStatement {
    pub result: Expr,
}

impl FromRule for EpsminStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::epsmin,
            "Expected `epsmin` rule when building EPSMIN statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let result_pair = inner
            .next()
            .context("Missing result variable in EPSMIN statement!")?;
        let result = Expr::from_rule(result_pair)
            .context("Failed to build result expression in EPSMIN statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in EPSMIN statement"))?;

        Ok(Some(EpsminStatement { result }))
    }
}

impl TranspileableStatement for EpsminStatement {
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

impl Transpile for EpsminStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let result_output = self.result.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in EPSMIN".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let result_ref = result_output.as_mut_ref();

        let serialization = format!("*{result_ref} = f64::MIN_POSITIVE;");

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
