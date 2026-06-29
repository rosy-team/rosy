//! # DASCL Statement (DA Scale Coefficients)
//!
//! Scales all coefficients of a DA vector array by a scalar factor.
//!
//! ## Syntax
//!
//! ```text
//! DASCL da_var scalar;
//! ```
//!
//! Arguments:
//! 1. `da_var` (DA array, in/out) — DA vector to scale in place
//! 2. `scalar` (RE)              — scale factor
//!
//! > **COSY note**: In COSY INFINITY, `DASCL` has different semantics — it scales the
//! > **i-th independent variable** by a factor and takes 4 arguments `(DA, i, a, result)`.
//! > Rosy's form scales **all coefficients** in place with 2 arguments.
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

/// AST node for `DASCL da_var scalar;`.
#[derive(Debug)]
pub struct DasclStatement {
    pub da_expr: Expr,
    pub scalar_expr: Expr,
}

impl FromRule for DasclStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dascl,
            "Expected `dascl` rule when building DASCL statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_pair = inner.next().context("Missing da_var parameter in DASCL!")?;
        let da_expr = Expr::from_rule(da_pair)
            .context("Failed to build da_var expression in DASCL")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_var expression in DASCL"))?;

        let scalar_pair = inner.next().context("Missing scalar parameter in DASCL!")?;
        let scalar_expr = Expr::from_rule(scalar_pair)
            .context("Failed to build scalar expression in DASCL")?
            .ok_or_else(|| anyhow::anyhow!("Expected scalar expression in DASCL"))?;

        Ok(Some(DasclStatement {
            da_expr,
            scalar_expr,
        }))
    }
}

impl TranspileableStatement for DasclStatement {
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

impl Transpile for DasclStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_output = self.da_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DASCL".to_string())
        })?;
        requested_variables.extend(da_output.requested_variables.iter().cloned());

        let scalar_output = self.scalar_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling scalar in DASCL".to_string())
        })?;
        requested_variables.extend(scalar_output.requested_variables.iter().cloned());

        let da_mut = da_output.as_mut_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_dascl({}, {} as f64)?;",
            da_mut,
            scalar_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
